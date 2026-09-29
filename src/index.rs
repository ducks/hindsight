//! The git index: one SQLite file per extracted repository, under
//! `<corpus>/raw/git/<name>.sqlite`. Raw in the schema's sense: derived
//! mechanically from history, never hand-edited, regenerable. Agents
//! query it at ingest time to write pattern and area pages; nobody reads
//! it at question time.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::blame;
use crate::corpus::Corpus;
use crate::git::{self, Commit};
use crate::symbols;

pub struct Index {
    conn: Connection,
}

/// One blame call: the fix sha, a path, and the old-file ranges the fix
/// removed from it.
type BlameItem = (String, String, Vec<(u32, u32)>);

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS commits (
  sha TEXT PRIMARY KEY, date TEXT NOT NULL, author TEXT NOT NULL,
  prefix TEXT, title TEXT NOT NULL, body TEXT NOT NULL, pr INTEGER
);
CREATE TABLE IF NOT EXISTS files (
  sha TEXT NOT NULL, path TEXT NOT NULL, status TEXT NOT NULL,
  added INTEGER NOT NULL, deleted INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS symbols (
  sha TEXT NOT NULL, path TEXT NOT NULL, kind TEXT NOT NULL,
  name TEXT NOT NULL, change TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS hunks (
  sha TEXT NOT NULL, path TEXT NOT NULL, old_start INTEGER NOT NULL, old_count INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS blame (
  sha TEXT NOT NULL, path TEXT NOT NULL, origin TEXT NOT NULL, lines INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS blamed (sha TEXT PRIMARY KEY);
CREATE INDEX IF NOT EXISTS commits_date ON commits(date);
CREATE INDEX IF NOT EXISTS commits_prefix ON commits(prefix);
CREATE INDEX IF NOT EXISTS files_sha ON files(sha);
CREATE INDEX IF NOT EXISTS files_path ON files(path);
CREATE INDEX IF NOT EXISTS symbols_sha ON symbols(sha);
CREATE INDEX IF NOT EXISTS symbols_kind_name ON symbols(kind, name);
CREATE INDEX IF NOT EXISTS hunks_sha ON hunks(sha);
CREATE INDEX IF NOT EXISTS blame_sha ON blame(sha);
CREATE INDEX IF NOT EXISTS blame_origin ON blame(origin);
";

/// Commit prefixes whose removed lines are corrections of earlier work.
const FIX_PREFIXES: &str = "'FIX', 'BUGFIX', 'SECURITY'";

pub fn path_for(c: &Corpus, name: &str) -> PathBuf {
    c.root
        .join("raw")
        .join("git")
        .join(format!("{name}.sqlite"))
}

impl Index {
    pub fn open(path: &Path) -> Result<Index, String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let conn =
            Connection::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=OFF;")
            .map_err(|e| e.to_string())?;
        conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
        Ok(Index { conn })
    }

    fn meta(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .ok()
    }

    /// Extract commits from `repo` into the index. Commits already present
    /// are skipped, so re-running after a fetch appends the new history.
    pub fn extract(
        &mut self,
        repo: &Path,
        rev: &str,
        limit: Option<usize>,
    ) -> Result<(usize, usize), String> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('repo', ?1)",
                params![repo.display().to_string()],
            )
            .map_err(|e| e.to_string())?;
        let mut known = self
            .conn
            .prepare("SELECT 1 FROM commits WHERE sha = ?1")
            .map_err(|e| e.to_string())?;
        let mut seen = 0;
        let mut added = 0;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        git::walk(repo, rev, limit, |c| {
            seen += 1;
            let exists = known.exists(params![c.sha]).unwrap_or(false);
            if !exists {
                if let Err(e) = insert(&tx, &c) {
                    eprintln!("skip {}: {e}", c.sha);
                } else {
                    added += 1;
                }
            }
            if seen % 500 == 0 {
                eprintln!("  {seen} commits read, {added} indexed");
            }
            true
        })?;
        drop(known);
        tx.commit().map_err(|e| e.to_string())?;
        Ok((seen, added))
    }

    /// Blame the lines each fix commit removed, newest first, skipping
    /// fixes already done. Files that carry no symbols (locales,
    /// lock files, fixtures) are skipped: their history is deep and
    /// their lines are not corrections of anything. `jobs` git processes run at once; results land
    /// in batches so an interrupted run keeps its progress.
    pub fn blame(
        &mut self,
        repo: Option<&Path>,
        limit: Option<usize>,
        jobs: usize,
    ) -> Result<(usize, usize), String> {
        let repo = match repo {
            Some(r) => r.to_path_buf(),
            None => PathBuf::from(
                self.meta("repo")
                    .ok_or("index has no repo path; pass --repo")?,
            ),
        };
        if !repo.join(".git").exists() && !repo.join("HEAD").exists() {
            return Err(format!(
                "{} is not a git repository; pass --repo",
                repo.display()
            ));
        }
        let sql = format!(
            "SELECT c.sha FROM commits c WHERE c.prefix IN ({FIX_PREFIXES}) AND NOT EXISTS (SELECT 1 FROM blamed b WHERE b.sha = c.sha) ORDER BY c.date DESC LIMIT ?1"
        );
        let todo: Vec<String> = {
            let mut st = self.conn.prepare(&sql).map_err(|e| e.to_string())?;
            let rows = st
                .query_map(params![limit.map(|l| l as i64).unwrap_or(-1)], |r| {
                    r.get::<_, String>(0)
                })
                .map_err(|e| e.to_string())?;
            rows.flatten().collect()
        };
        let total = todo.len();
        let mut pairs = 0usize;
        let mut done = 0usize;
        for batch in todo.chunks(200) {
            // Work items: one blame call per (fix, path) with all its ranges.
            let mut items: Vec<BlameItem> = Vec::new();
            {
                let mut st = self
                    .conn
                    .prepare("SELECT path, old_start, old_count FROM hunks WHERE sha = ?1 ORDER BY path, old_start")
                    .map_err(|e| e.to_string())?;
                for sha in batch {
                    let rows = st
                        .query_map(params![sha], |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, u32>(1)?,
                                r.get::<_, u32>(2)?,
                            ))
                        })
                        .map_err(|e| e.to_string())?;
                    for (path, start, count) in rows.flatten() {
                        if symbols::skip_path(&path) {
                            continue;
                        }
                        match items.last_mut() {
                            Some((s, p, ranges)) if s == sha && *p == path => {
                                ranges.push((start, count))
                            }
                            _ => items.push((sha.clone(), path, vec![(start, count)])),
                        }
                    }
                }
            }
            let results = run_blames(&repo, &items, jobs);
            let tx = self
                .conn
                .unchecked_transaction()
                .map_err(|e| e.to_string())?;
            {
                let mut ins = tx
                    .prepare_cached(
                        "INSERT INTO blame (sha, path, origin, lines) VALUES (?1, ?2, ?3, ?4)",
                    )
                    .map_err(|e| e.to_string())?;
                for (sha, path, origin, lines) in &results {
                    ins.execute(params![sha, path, origin, lines])
                        .map_err(|e| e.to_string())?;
                    pairs += 1;
                }
                let mut mark = tx
                    .prepare_cached("INSERT OR IGNORE INTO blamed (sha) VALUES (?1)")
                    .map_err(|e| e.to_string())?;
                for sha in batch {
                    mark.execute(params![sha]).map_err(|e| e.to_string())?;
                }
            }
            tx.commit().map_err(|e| e.to_string())?;
            done += batch.len();
            eprintln!("  {done}/{total} fixes blamed, {pairs} origin rows");
        }
        Ok((total, pairs))
    }

    pub fn stats(&self) -> Result<String, String> {
        let mut out = String::new();
        let q = |sql: &str| -> Result<Vec<(String, i64)>, String> {
            let mut st = self.conn.prepare(sql).map_err(|e| e.to_string())?;
            let rows = st
                .query_map([], |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?
                            .unwrap_or_else(|| "(none)".into()),
                        r.get(1)?,
                    ))
                })
                .map_err(|e| e.to_string())?;
            Ok(rows.flatten().collect())
        };
        let (n, from, to): (i64, String, String) = self
            .conn
            .query_row(
                "SELECT count(*), min(date), max(date) FROM commits",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|e| e.to_string())?;
        out.push_str(&format!("{n} commits, {from} to {to}\n\nby prefix:\n"));
        for (k, v) in
            q("SELECT prefix, count(*) FROM commits GROUP BY prefix ORDER BY 2 DESC LIMIT 12")?
        {
            out.push_str(&format!("  {v:>6}  {k}\n"));
        }
        out.push_str("\nsymbols by kind:\n");
        for (k, v) in q("SELECT kind, count(*) FROM symbols GROUP BY kind ORDER BY 2 DESC")? {
            out.push_str(&format!("  {v:>6}  {k}\n"));
        }
        let (blamed, fixes): (i64, i64) = self
            .conn
            .query_row(
                &format!("SELECT (SELECT count(*) FROM blamed), (SELECT count(*) FROM commits WHERE prefix IN ({FIX_PREFIXES}))"),
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        out.push_str(&format!("\nblame: {blamed} of {fixes} fix commits\n"));
        Ok(out)
    }

    pub fn show(&self, sha_prefix: &str) -> Result<String, String> {
        let like = format!("{sha_prefix}%");
        let (sha, date, author, prefix, title, body, pr): (
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            Option<i64>,
        ) = self
            .conn
            .query_row(
                "SELECT sha, date, author, prefix, title, body, pr FROM commits WHERE sha LIKE ?1",
                params![like],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                    ))
                },
            )
            .map_err(|_| format!("no commit matching {sha_prefix}"))?;
        let mut out = format!(
            "{} {} {}\n{}{}{}\n",
            &sha[..12],
            date,
            author,
            prefix.map(|p| format!("{p}: ")).unwrap_or_default(),
            title,
            pr.map(|n| format!(" (#{n})")).unwrap_or_default()
        );
        if !body.is_empty() {
            out.push_str(&format!("\n{body}\n"));
        }
        out.push_str("\nfiles:\n");
        let mut st = self
            .conn
            .prepare("SELECT status, path, added, deleted FROM files WHERE sha = ?1 ORDER BY path")
            .map_err(|e| e.to_string())?;
        for row in st
            .query_map(params![sha], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .flatten()
        {
            out.push_str(&format!("  {} {} (+{} -{})\n", row.0, row.1, row.2, row.3));
        }
        out.push_str("\nsymbols:\n");
        let mut st = self
            .conn
            .prepare(
                "SELECT change, kind, name, path FROM symbols WHERE sha = ?1 ORDER BY kind, name",
            )
            .map_err(|e| e.to_string())?;
        for row in st
            .query_map(params![sha], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let mark = if row.0 == "add" { '+' } else { '-' };
            out.push_str(&format!("  {mark} {:<12} {}  ({})\n", row.1, row.2, row.3));
        }
        let origins = self.origins_of(&sha)?;
        if !origins.is_empty() {
            out.push_str("\nremoved lines written by:\n");
            for o in origins {
                out.push_str(&format!("  {o}\n"));
            }
        }
        let undone = self.undone_rows(&sha, 10)?;
        if !undone.is_empty() {
            out.push_str("\nlater fixes on its lines:\n");
            for u in undone {
                out.push_str(&format!("  {u}\n"));
            }
        }
        Ok(out)
    }

    /// Commits matching every filter. `added` entries are `kind` or
    /// `kind:glob` on the symbol name; `touches` are path globs.
    pub fn find(
        &self,
        prefix: Option<&str>,
        added: &[String],
        touches: &[String],
        limit: usize,
    ) -> Result<String, String> {
        let mut sql =
            String::from("SELECT c.sha, c.date, c.prefix, c.title, c.pr FROM commits c WHERE 1=1");
        let mut args: Vec<String> = Vec::new();
        if let Some(p) = prefix {
            sql.push_str(" AND c.prefix = ?");
            args.push(p.to_string());
        }
        for a in added {
            let (kind, glob) = match a.split_once(':') {
                Some((k, g)) => (k, g),
                None => (a.as_str(), "*"),
            };
            sql.push_str(" AND EXISTS (SELECT 1 FROM symbols s WHERE s.sha = c.sha AND s.change = 'add' AND s.kind = ? AND s.name GLOB ?)");
            args.push(kind.to_string());
            args.push(glob.to_string());
        }
        for t in touches {
            sql.push_str(
                " AND EXISTS (SELECT 1 FROM files f WHERE f.sha = c.sha AND f.path GLOB ?)",
            );
            args.push(t.clone());
        }
        sql.push_str(" ORDER BY c.date DESC LIMIT ?");
        args.push(limit.to_string());

        let mut st = self.conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = st
            .query_map(rusqlite::params_from_iter(args.iter()), |r| {
                Ok(line(
                    r.get::<_, String>(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let out: Vec<String> = rows.flatten().collect();
        if out.is_empty() {
            return Ok("no commits match".into());
        }
        Ok(out.join("\n"))
    }

    /// Files that change in the same commits as files matching `glob`.
    pub fn cochange(&self, glob: &str, limit: usize) -> Result<String, String> {
        let mut st = self
            .conn
            .prepare(
                "SELECT f2.path, count(DISTINCT f1.sha) AS n FROM files f1 JOIN files f2 ON f1.sha = f2.sha AND f2.path != f1.path
                 WHERE f1.path GLOB ?1 GROUP BY f2.path ORDER BY n DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![glob, limit as i64], |r| {
                Ok(format!(
                    "{:>5}  {}",
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(0)?
                ))
            })
            .map_err(|e| e.to_string())?;
        let out: Vec<String> = rows.flatten().collect();
        if out.is_empty() {
            return Ok("no files match".into());
        }
        Ok(out.join("\n"))
    }

    /// Correction pairs: each fix with the commits that wrote the lines
    /// it removed, ranked by lines. `window` limits how far back the
    /// origin may be; `touches` restricts to paths matching a glob.
    pub fn fixes(
        &self,
        window: Option<u32>,
        touches: Option<&str>,
        limit: usize,
    ) -> Result<String, String> {
        self.require_blame()?;
        let glob = touches.unwrap_or("*");
        let sql = "SELECT fix.sha, fix.date, fix.prefix, fix.title, fix.pr,
                    o.sha, o.date, o.prefix, o.title, o.pr, b.origin,
                    sum(b.lines) AS n, group_concat(DISTINCT b.path)
             FROM blame b
             JOIN commits fix ON fix.sha = b.sha
             LEFT JOIN commits o ON o.sha = b.origin
             WHERE b.path GLOB ?1
               AND (?2 < 0 OR (o.date IS NOT NULL AND julianday(fix.date) - julianday(o.date) <= ?2))
             GROUP BY b.sha, b.origin
             ORDER BY fix.date DESC, n DESC LIMIT ?3";
        let mut st = self.conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = st
            .query_map(
                params![glob, window.map(|w| w as i64).unwrap_or(-1), limit as i64],
                |r| {
                    let fix = line(
                        r.get::<_, String>(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                    );
                    let origin = match r.get::<_, Option<String>>(5)? {
                        Some(sha) => line(sha, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?),
                        None => format!("{} (not indexed)", &r.get::<_, String>(10)?[..12]),
                    };
                    Ok(format!(
                        "{fix}\n    {} lines from {origin}\n    in {}",
                        r.get::<_, i64>(11)?,
                        r.get::<_, String>(12)?.replace(',', ", ")
                    ))
                },
            )
            .map_err(|e| e.to_string())?;
        let out: Vec<String> = rows.flatten().collect();
        if out.is_empty() {
            return Ok("no fix pairs found".into());
        }
        Ok(out.join("\n"))
    }

    /// Fixes that removed lines a commit wrote: what it got wrong.
    pub fn undone(&self, sha_prefix: &str, limit: usize) -> Result<String, String> {
        self.require_blame()?;
        let rows = self.undone_rows(sha_prefix, limit)?;
        if rows.is_empty() {
            return Ok("no later fix touched its lines".into());
        }
        Ok(rows.join("\n"))
    }

    fn undone_rows(&self, sha_prefix: &str, limit: usize) -> Result<Vec<String>, String> {
        let mut st = self
            .conn
            .prepare(
                "SELECT fix.sha, fix.date, fix.prefix, fix.title, fix.pr, sum(b.lines) AS n, group_concat(DISTINCT b.path)
                 FROM blame b JOIN commits fix ON fix.sha = b.sha
                 WHERE b.origin LIKE ?1 GROUP BY b.sha ORDER BY n DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![format!("{sha_prefix}%"), limit as i64], |r| {
                Ok(format!(
                    "{} [{} lines in {}]",
                    line(
                        r.get::<_, String>(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?
                    ),
                    r.get::<_, i64>(5)?,
                    r.get::<_, String>(6)?.replace(',', ", ")
                ))
            })
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().collect())
    }

    fn origins_of(&self, sha: &str) -> Result<Vec<String>, String> {
        let mut st = self
            .conn
            .prepare(
                "SELECT b.origin, o.date, o.prefix, o.title, o.pr, sum(b.lines) AS n
                 FROM blame b LEFT JOIN commits o ON o.sha = b.origin
                 WHERE b.sha = ?1 GROUP BY b.origin ORDER BY n DESC LIMIT 10",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![sha], |r| {
                let origin: String = r.get(0)?;
                let n: i64 = r.get(5)?;
                Ok(match r.get::<_, Option<String>>(1)? {
                    Some(date) => format!(
                        "{} [{n} lines]",
                        line(origin, date, r.get(2)?, r.get(3)?, r.get(4)?)
                    ),
                    None => format!("{} (not indexed) [{n} lines]", &origin[..12]),
                })
            })
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().collect())
    }

    fn require_blame(&self) -> Result<(), String> {
        let n: i64 = self
            .conn
            .query_row("SELECT count(*) FROM blamed", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("no blame data yet; run: hindsight git <name> blame".into());
        }
        Ok(())
    }
}

/// Run the blame calls `jobs` at a time. Each item is (fix sha, path,
/// removed ranges); each result row is (fix sha, path, origin, lines).
fn run_blames(repo: &Path, items: &[BlameItem], jobs: usize) -> Vec<(String, String, String, u32)> {
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(String, String, String, u32)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some((sha, path, ranges)) = items.get(i) else {
                    break;
                };
                match blame::blame_ranges(repo, sha, path, ranges) {
                    Ok(origins) => {
                        let mut out = results.lock().unwrap();
                        for (origin, lines) in origins {
                            out.push((sha.clone(), path.clone(), origin, lines));
                        }
                    }
                    Err(e) => eprintln!("  {e}"),
                }
            });
        }
    });
    results.into_inner().unwrap()
}

fn line(
    sha: String,
    date: String,
    prefix: Option<String>,
    title: String,
    pr: Option<i64>,
) -> String {
    format!(
        "{} {} {}{}{}",
        &sha[..12],
        date,
        prefix.map(|p| format!("{p}: ")).unwrap_or_default(),
        title,
        pr.map(|n| format!(" (#{n})")).unwrap_or_default()
    )
}

fn insert(conn: &Connection, c: &Commit) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO commits (sha, date, author, prefix, title, body, pr) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![c.sha, c.date, c.author, c.prefix, c.title, c.body, c.pr],
    )?;
    let mut fs = conn.prepare_cached(
        "INSERT INTO files (sha, path, status, added, deleted) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    let mut hs = conn.prepare_cached(
        "INSERT INTO hunks (sha, path, old_start, old_count) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for f in &c.files {
        fs.execute(params![
            c.sha,
            f.path,
            f.status.to_string(),
            f.added,
            f.deleted
        ])?;
        for (start, count) in &f.removed {
            hs.execute(params![c.sha, f.path, start, count])?;
        }
    }
    let mut ss = conn.prepare_cached(
        "INSERT INTO symbols (sha, path, kind, name, change) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for s in &c.symbols {
        ss.execute(params![c.sha, s.path, s.kind, s.name, s.change])?;
    }
    Ok(())
}
