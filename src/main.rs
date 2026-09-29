mod blame;
mod corpus;
mod git;
mod index;
mod mcp;
mod symbols;

use std::path::PathBuf;

use corpus::Corpus;

const USAGE: &str = "\
hindsight - incident knowledge base (llm-wiki) tools

Usage: hindsight [--data <dir>] <command>

Commands:
  serve   Run the MCP server (stdio) over the corpus
  init    Scaffold a corpus directory (raw/ + wiki/)
  lint    Check wiki links and index coverage
  path    Print the resolved corpus path

  extract <repo> [--name <n>] [--rev <rev>] [--limit <n>]
          Index a git repository's history into raw/git/<name>.sqlite.
          Re-running appends commits not yet indexed.
  git <name> stats
  git <name> show <sha>
  git <name> find [--prefix P] [--added kind[:glob]]... [--touches glob]... [--limit n]
  git <name> cochange <glob> [--limit n]
  git <name> blame [--limit n] [--jobs n] [--repo path]
          Blame the lines each fix removed onto the commits that wrote
          them; incremental, newest fixes first.
  git <name> fixes [--touches glob] [--window days] [--limit n]
  git <name> undone <sha>
          Correction pairs from the blame data: what each fix corrected,
          and which later fixes corrected a given commit.

Corpus resolution order:
  --data flag, $HINDSIGHT_DATA, $XDG_DATA_HOME/hindsight,
  ~/.local/share/hindsight";

fn main() {
    let mut data: Option<PathBuf> = None;
    let mut rest: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data" => data = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                println!("{}", USAGE);
                return;
            }
            other => rest.push(other.to_string()),
        }
    }

    let corpus = Corpus::resolve(data);

    let result = match rest.first().map(String::as_str) {
        Some("serve") => {
            mcp::serve(&corpus);
            Ok(())
        }
        Some("init") => {
            corpus::init(&corpus);
            Ok(())
        }
        Some("lint") => {
            if !corpus::lint(&corpus) {
                std::process::exit(1);
            }
            Ok(())
        }
        Some("path") => {
            println!("{}", corpus.root.display());
            Ok(())
        }
        Some("extract") => extract(&corpus, &rest[1..]),
        Some("git") => git_query(&corpus, &rest[1..]),
        _ => {
            eprintln!("{}", USAGE);
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

/// Pull `--flag value` pairs (repeatable) out of args; returns positionals.
fn flags(args: &[String], names: &[&str]) -> (Vec<String>, Vec<(String, String)>) {
    let mut pos = Vec::new();
    let mut found = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if names.contains(&args[i].as_str()) {
            if let Some(v) = args.get(i + 1) {
                found.push((args[i].clone(), v.clone()));
            }
            i += 2;
        } else {
            pos.push(args[i].clone());
            i += 1;
        }
    }
    (pos, found)
}

fn flag<'a>(found: &'a [(String, String)], name: &str) -> Option<&'a str> {
    found
        .iter()
        .rev()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn flag_all(found: &[(String, String)], name: &str) -> Vec<String> {
    found
        .iter()
        .filter(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
        .collect()
}

fn extract(corpus: &Corpus, args: &[String]) -> Result<(), String> {
    let (pos, found) = flags(args, &["--name", "--rev", "--limit"]);
    let repo = PathBuf::from(pos.first().ok_or("extract needs a repository path")?);
    let repo = repo
        .canonicalize()
        .map_err(|e| format!("{}: {e}", repo.display()))?;
    let name = flag(&found, "--name")
        .map(String::from)
        .or_else(|| repo.file_name().map(|n| n.to_string_lossy().to_string()))
        .ok_or("cannot name the repository; pass --name")?;
    let rev = flag(&found, "--rev").unwrap_or("HEAD");
    let limit = flag(&found, "--limit")
        .map(|l| l.parse::<usize>().map_err(|_| "--limit must be a number"))
        .transpose()?;

    let path = index::path_for(corpus, &name);
    let mut idx = index::Index::open(&path)?;
    eprintln!("indexing {} into {}", repo.display(), path.display());
    let (seen, added) = idx.extract(&repo, rev, limit)?;
    println!("{seen} commits read, {added} newly indexed");
    Ok(())
}

fn git_query(corpus: &Corpus, args: &[String]) -> Result<(), String> {
    let name = args
        .first()
        .ok_or("git needs the repository name (see extract)")?;
    let path = index::path_for(corpus, name);
    if !path.exists() {
        return Err(format!(
            "no index for {name}; run: hindsight extract <repo> --name {name}"
        ));
    }
    let mut idx = index::Index::open(&path)?;
    let (pos, found) = flags(
        &args[1..],
        &[
            "--prefix",
            "--added",
            "--touches",
            "--limit",
            "--window",
            "--jobs",
            "--repo",
        ],
    );
    let limit = flag(&found, "--limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(20);
    let out = match pos.first().map(String::as_str) {
        Some("stats") => idx.stats()?,
        Some("show") => idx.show(pos.get(1).ok_or("show needs a sha")?)?,
        Some("find") => idx.find(
            flag(&found, "--prefix"),
            &flag_all(&found, "--added"),
            &flag_all(&found, "--touches"),
            limit,
        )?,
        Some("cochange") => idx.cochange(pos.get(1).ok_or("cochange needs a path glob")?, limit)?,
        Some("blame") => {
            let jobs = flag(&found, "--jobs")
                .and_then(|j| j.parse().ok())
                .unwrap_or(8);
            let repo = flag(&found, "--repo").map(PathBuf::from);
            let limit = flag(&found, "--limit").and_then(|l| l.parse().ok());
            let (fixes, rows) = idx.blame(repo.as_deref(), limit, jobs)?;
            format!("{fixes} fix commits blamed, {rows} origin rows")
        }
        Some("fixes") => {
            let window = flag(&found, "--window").and_then(|w| w.parse().ok());
            idx.fixes(window, flag(&found, "--touches"), limit)?
        }
        Some("undone") => idx.undone(pos.get(1).ok_or("undone needs a sha")?, limit)?,
        _ => {
            return Err("git subcommands: stats, show, find, cochange, blame, fixes, undone".into())
        }
    };
    println!("{out}");
    Ok(())
}
