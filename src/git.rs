//! Stream `git log -p` and turn each commit into a record: header,
//! files touched, and the symbols its added and removed lines name.
//! Records are handed to a callback as they are parsed so the whole
//! history never sits in memory.

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use regex::Regex;

use crate::symbols;

pub struct FileChange {
    pub path: String,
    pub status: char,
    pub added: u32,
    pub deleted: u32,
    /// (start, count) ranges of the old file this change removed.
    pub removed: Vec<(u32, u32)>,
}

pub struct SymbolChange {
    pub path: String,
    pub kind: &'static str,
    pub name: String,
    pub change: &'static str,
}

pub struct Commit {
    pub sha: String,
    pub author: String,
    pub date: String,
    pub prefix: Option<String>,
    pub title: String,
    pub body: String,
    pub pr: Option<u32>,
    pub files: Vec<FileChange>,
    pub symbols: Vec<SymbolChange>,
}

const RS: u8 = 0x1e;
const US: u8 = 0x1f;

/// Walk commits reachable from `rev` (newest first), at most `limit` when
/// given, calling `f` for each. Stops early when `f` returns false.
pub fn walk(
    repo: &Path,
    rev: &str,
    limit: Option<usize>,
    mut f: impl FnMut(Commit) -> bool,
) -> Result<usize, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo)
        .args([
            "log",
            "-p",
            "--no-color",
            "--no-renames",
            "--date=short",
            "--first-parent",
        ])
        .arg("--format=%x1e%H%x00%an%x00%ad%x00%s%x00%b%x1f");
    if let Some(n) = limit {
        cmd.arg(format!("-{n}"));
    }
    cmd.arg(rev);
    cmd.arg("--").arg(".").args([
        ":!pnpm-lock.yaml",
        ":!yarn.lock",
        ":!Gemfile.lock",
        ":!package-lock.json",
    ]);
    cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());

    let mut child = cmd.spawn().map_err(|e| format!("cannot run git: {e}"))?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let mut reader = BufReader::with_capacity(1 << 20, stdout);

    let subject = Regex::new(r"^([A-Z][A-Z0-9_]+):\s*(.*)$").unwrap();
    let pr_ref = Regex::new(r"\s*\(#(\d+)\)\s*$").unwrap();

    let mut buf = Vec::new();
    let mut count = 0;
    let mut first = true;
    loop {
        buf.clear();
        let n = reader.read_until(RS, &mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if buf.last() == Some(&RS) {
            buf.pop();
        }
        if first {
            // Everything before the first record separator is empty.
            first = false;
            if buf.is_empty() {
                continue;
            }
        }
        if buf.is_empty() {
            continue;
        }
        let Some(commit) = parse_record(&buf, &subject, &pr_ref) else {
            continue;
        };
        count += 1;
        if !f(commit) {
            let _ = child.kill();
            break;
        }
    }
    let _ = child.wait();
    Ok(count)
}

fn parse_record(raw: &[u8], subject: &Regex, pr_ref: &Regex) -> Option<Commit> {
    let split = raw.iter().position(|b| *b == US)?;
    let header = String::from_utf8_lossy(&raw[..split]);
    let diff = String::from_utf8_lossy(&raw[split + 1..]);

    let mut parts = header.splitn(5, '\0');
    let sha = parts.next()?.trim().to_string();
    let author = parts.next()?.to_string();
    let date = parts.next()?.to_string();
    let mut title = parts.next()?.to_string();
    let body = parts.next().unwrap_or("").trim().to_string();

    let mut pr = None;
    if let Some(c) = pr_ref.captures(&title) {
        pr = c[1].parse().ok();
        title = pr_ref.replace(&title, "").to_string();
    }
    let mut prefix = None;
    if let Some(c) = subject.captures(&title) {
        prefix = Some(c[1].to_string());
        title = c[2].to_string();
    }

    let mut commit = Commit {
        sha,
        author,
        date,
        prefix,
        title,
        body,
        pr,
        files: Vec::new(),
        symbols: Vec::new(),
    };
    parse_diff(&diff, &mut commit);
    Some(commit)
}

fn parse_diff(diff: &str, commit: &mut Commit) {
    let mut seen: BTreeSet<(String, &'static str, String, &'static str)> = BTreeSet::new();
    let mut current: Option<usize> = None;
    let mut skip = false;

    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            let path = match rest.find(" b/") {
                Some(i) => rest[i + 3..].to_string(),
                None => rest.to_string(),
            };
            skip = symbols::skip_path(&path);
            commit.files.push(FileChange {
                path,
                status: 'M',
                added: 0,
                deleted: 0,
                removed: Vec::new(),
            });
            current = Some(commit.files.len() - 1);
            continue;
        }
        let Some(idx) = current else { continue };
        if line.starts_with("new file mode") {
            commit.files[idx].status = 'A';
            continue;
        }
        if line.starts_with("deleted file mode") {
            commit.files[idx].status = 'D';
            continue;
        }
        if let Some(range) = hunk_removed(line) {
            commit.files[idx].removed.push(range);
            continue;
        }
        if line.starts_with("+++") || line.starts_with("---") || line.starts_with("Binary files") {
            continue;
        }
        let (change, content) = if let Some(c) = line.strip_prefix('+') {
            commit.files[idx].added += 1;
            ("add", c)
        } else if let Some(c) = line.strip_prefix('-') {
            commit.files[idx].deleted += 1;
            ("del", c)
        } else {
            continue;
        };
        if skip {
            continue;
        }
        let path = &commit.files[idx].path;
        for (kind, name) in symbols::extract(path, content) {
            if seen.insert((path.clone(), kind, name.clone(), change)) {
                commit.symbols.push(SymbolChange {
                    path: path.clone(),
                    kind,
                    name,
                    change,
                });
            }
        }
    }

    // A migration is a symbol in its own right: the file's name is the
    // schema change's name.
    let migrations: Vec<String> = commit
        .files
        .iter()
        .filter(|f| f.status == 'A' && f.path.contains("db/migrate/") && f.path.ends_with(".rb"))
        .map(|f| f.path.clone())
        .collect();
    for path in migrations {
        let name = path
            .rsplit('/')
            .next()
            .unwrap_or(&path)
            .trim_end_matches(".rb")
            .to_string();
        commit.symbols.push(SymbolChange {
            path,
            kind: "migration",
            name,
            change: "add",
        });
    }
}

/// The old-file range a hunk header removes: `@@ -a,b +c,d @@` with
/// b defaulting to 1 and a zero count meaning pure addition.
fn hunk_removed(line: &str) -> Option<(u32, u32)> {
    let rest = line.strip_prefix("@@ -")?;
    let end = rest.find(' ')?;
    let old = &rest[..end];
    let (start, count) = match old.split_once(',') {
        Some((s, c)) => (s.parse().ok()?, c.parse().ok()?),
        None => (old.parse().ok()?, 1),
    };
    if count == 0 {
        return None;
    }
    Some((start, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_record() {
        assert_eq!(
            hunk_removed("@@ -2185,2 +2185,2 @@ class Topic"),
            Some((2185, 2))
        );
        assert_eq!(hunk_removed("@@ -7 +7,3 @@"), Some((7, 1)));
        assert_eq!(hunk_removed("@@ -0,0 +1 @@"), None);
        let subject = Regex::new(r"^([A-Z][A-Z0-9_]+):\s*(.*)$").unwrap();
        let pr_ref = Regex::new(r"\s*\(#(\d+)\)\s*$").unwrap();
        let raw = "abc123\0Ann\02026-09-29\0FIX: Stop the drift (#44134)\0Why it drifted.\n\x1f\ndiff --git a/app/models/post.rb b/app/models/post.rb\nindex 1..2 100644\n--- a/app/models/post.rb\n+++ b/app/models/post.rb\n@@ -1 +1 @@\n-  def old_cook\n+  def cook\n+    SiteSetting.max_post_length\ndiff --git a/db/migrate/20260929_add_x.rb b/db/migrate/20260929_add_x.rb\nnew file mode 100644\n--- /dev/null\n+++ b/db/migrate/20260929_add_x.rb\n@@ -0,0 +1 @@\n+    add_column :posts, :x, :text\n";
        let c = parse_record(raw.as_bytes(), &subject, &pr_ref).unwrap();
        assert_eq!(c.sha, "abc123");
        assert_eq!(c.prefix.as_deref(), Some("FIX"));
        assert_eq!(c.title, "Stop the drift");
        assert_eq!(c.pr, Some(44134));
        assert_eq!(c.body, "Why it drifted.");
        assert_eq!(c.files.len(), 2);
        assert_eq!(c.files[0].added, 2);
        assert_eq!(c.files[0].deleted, 1);
        assert_eq!(c.files[1].status, 'A');
        assert_eq!(c.files[0].removed, vec![(1, 1)]);
        assert!(c.files[1].removed.is_empty());
        let names: Vec<(&str, &str, &str)> = c
            .symbols
            .iter()
            .map(|s| (s.kind, s.name.as_str(), s.change))
            .collect();
        assert!(names.contains(&("def", "old_cook", "del")));
        assert!(names.contains(&("def", "cook", "add")));
        assert!(names.contains(&("setting_use", "max_post_length", "add")));
        assert!(names.contains(&("schema", "add_column posts", "add")));
        assert!(names.contains(&("migration", "20260929_add_x", "add")));
    }
}
