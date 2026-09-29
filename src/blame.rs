//! Who wrote the lines a fix removed. `git blame` on the fix's parent,
//! restricted to the removed ranges, names the commit that introduced
//! each line; that is a correction pair by construction, weighted by
//! how many lines the fix blamed on each origin.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// (origin sha, lines blamed on it) for the given old-file ranges of
/// `path` as it stood in the parent of `sha`. Whitespace-only changes
/// are looked through so reformatting commits are not named as origins.
pub fn blame_ranges(
    repo: &Path,
    sha: &str,
    path: &str,
    ranges: &[(u32, u32)],
) -> Result<Vec<(String, u32)>, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).args(["blame", "-w", "--porcelain"]);
    for (start, count) in ranges {
        cmd.arg("-L").arg(format!("{start},+{count}"));
    }
    cmd.arg(format!("{sha}^")).arg("--").arg(path);
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run git blame: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git blame {} {}: {}",
            &sha[..12],
            path,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(parse_porcelain(&String::from_utf8_lossy(&out.stdout)))
}

/// Count content lines per origin sha in `git blame --porcelain` output.
/// A group starts with `<sha> <orig> <final> [<n>]`; its content line
/// starts with a tab; anything else is metadata.
pub fn parse_porcelain(text: &str) -> Vec<(String, u32)> {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if line.starts_with('\t') {
            if let Some(sha) = &current {
                *counts.entry(sha.clone()).or_insert(0) += 1;
            }
            continue;
        }
        let mut parts = line.split(' ');
        if let (Some(sha), Some(a), Some(b)) = (parts.next(), parts.next(), parts.next()) {
            if sha.len() == 40
                && sha.bytes().all(|c| c.is_ascii_hexdigit())
                && a.parse::<u32>().is_ok()
                && b.parse::<u32>().is_ok()
            {
                current = Some(sha.to_string());
            }
        }
    }
    let mut out: Vec<(String, u32)> = counts.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_lines_per_origin() {
        let text = "\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 10 10 2
author Ann
summary FEATURE: thing
\tline one
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 11 11
\tline two
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 5 12 1
author Bob
\tline three
";
        assert_eq!(
            parse_porcelain(text),
            vec![("a".repeat(40), 2), ("b".repeat(40), 1)]
        );
    }
}
