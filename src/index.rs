//! The git index: one SQLite file per extracted repository, under
//! `<corpus>/raw/git/<name>.sqlite`. Raw in the schema's sense: derived
//! mechanically from history, never hand-edited, regenerable. Agents
//! query it at ingest time to write pattern and area pages; nobody reads
//! it at question time.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};

use crate::corpus::Corpus;
use crate::git::{self, Commit};

pub struct Index {
    conn: Connection,
}

const SCHEMA: &str = "
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
CREATE INDEX IF NOT EXISTS commits_date ON commits(date);
CREATE INDEX IF NOT EXISTS commits_prefix ON commits(prefix);
CREATE INDEX IF NOT EXISTS files_sha ON files(sha);
CREATE INDEX IF NOT EXISTS files_path ON files(path);
CREATE INDEX IF NOT EXISTS symbols_sha ON symbols(sha);
CREATE INDEX IF NOT EXISTS symbols_kind_name ON symbols(kind, name);
";

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

    /// Extract commits from `repo` into the index. Commits already present
    /// are skipped, so re-running after a fetch appends the new history.
    pub fn extract(
        &mut self,
        repo: &Path,
        rev: &str,
        limit: Option<usize>,
    ) -> Result<(usize, usize), String> {
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

    /// FIX commits paired with the earlier feature-ish commit that touched
    /// the same file within `window` days: the correction pairs.
    pub fn fixes(
        &self,
        window: u32,
        touches: Option<&str>,
        limit: usize,
    ) -> Result<String, String> {
        let glob = touches.unwrap_or("*");
        let mut st = self
            .conn
            .prepare(
                "SELECT fix.sha, fix.date, fix.title, fix.pr, feat.sha, feat.date, feat.prefix, feat.title, feat.pr, f1.path
                 FROM commits fix
                 JOIN files f1 ON f1.sha = fix.sha
                 JOIN files f2 ON f2.path = f1.path AND f2.sha != fix.sha
                 JOIN commits feat ON feat.sha = f2.sha
                 WHERE fix.prefix = 'FIX' AND feat.prefix IN ('FEATURE', 'UX', 'DEV', 'PERF')
                   AND julianday(fix.date) - julianday(feat.date) BETWEEN 0 AND ?1
                   AND f1.path GLOB ?2 AND f1.path NOT LIKE '%/locales/%'
                 GROUP BY fix.sha, feat.sha ORDER BY fix.date DESC LIMIT ?3",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![window as i64, glob, limit as i64], |r| {
                Ok(format!(
                    "{}\n    fixes {}\n    via {}",
                    line(
                        r.get::<_, String>(0)?,
                        r.get(1)?,
                        Some("FIX".into()),
                        r.get(2)?,
                        r.get(3)?
                    ),
                    line(
                        r.get::<_, String>(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                        r.get(8)?
                    ),
                    r.get::<_, String>(9)?
                ))
            })
            .map_err(|e| e.to_string())?;
        let out: Vec<String> = rows.flatten().collect();
        if out.is_empty() {
            return Ok("no fix pairs found".into());
        }
        Ok(out.join("\n"))
    }
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
    for f in &c.files {
        fs.execute(params![
            c.sha,
            f.path,
            f.status.to_string(),
            f.added,
            f.deleted
        ])?;
    }
    let mut ss = conn.prepare_cached(
        "INSERT INTO symbols (sha, path, kind, name, change) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for s in &c.symbols {
        ss.execute(params![c.sha, s.path, s.kind, s.name, s.change])?;
    }
    Ok(())
}
