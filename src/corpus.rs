use std::fs;
use std::path::{Component, Path, PathBuf};

pub struct Corpus {
    pub root: PathBuf,
}

impl Corpus {
    /// Resolution order: explicit flag, $HINDSIGHT_DATA,
    /// $XDG_DATA_HOME/hindsight, ~/.local/share/hindsight.
    pub fn resolve(flag: Option<PathBuf>) -> Corpus {
        let root = flag
            .or_else(|| std::env::var_os("HINDSIGHT_DATA").map(PathBuf::from))
            .or_else(|| {
                std::env::var_os("XDG_DATA_HOME").map(|d| PathBuf::from(d).join("hindsight"))
            })
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/hindsight"))
            })
            .unwrap_or_else(|| PathBuf::from("."));
        Corpus { root }
    }

    pub fn wiki(&self) -> PathBuf {
        self.root.join("wiki")
    }

    /// Read a wiki page by wiki-relative path (e.g. "patterns/redis-oom.md").
    pub fn get_page(&self, rel: &str) -> Result<String, String> {
        let p = Path::new(rel);
        let escapes = p.is_absolute()
            || p.components()
                .any(|c| matches!(c, Component::ParentDir | Component::RootDir));
        if escapes || !rel.ends_with(".md") {
            return Err(format!("invalid page path: {}", rel));
        }
        let full = self.wiki().join(p);
        fs::read_to_string(&full).map_err(|e| format!("cannot read {}: {}", rel, e))
    }

    /// List pages of a kind. "index" returns index.md itself.
    pub fn list(&self, kind: &str) -> Result<String, String> {
        match kind {
            "index" => self.get_page("index.md"),
            "incidents" | "patterns" | "systems" => {
                let dir = self.wiki().join(kind);
                let rd = fs::read_dir(&dir).map_err(|e| format!("cannot list {}: {}", kind, e))?;
                let mut paths: Vec<PathBuf> = rd
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().map_or(false, |x| x == "md"))
                    .collect();
                paths.sort();
                let mut out = Vec::new();
                for p in paths {
                    let content = fs::read_to_string(&p).unwrap_or_default();
                    let title = page_title(&content).unwrap_or_default();
                    let name = p.file_name().map(|n| n.to_string_lossy().to_string());
                    if let Some(name) = name {
                        out.push(format!("{}/{} - {}", kind, name, title));
                    }
                }
                Ok(out.join("\n"))
            }
            other => Err(format!(
                "unknown kind: {} (expected incidents|patterns|systems|index)",
                other
            )),
        }
    }

    /// Case-insensitive term search across all wiki pages.
    /// Pages containing ALL terms rank first; any-term matches are the
    /// fallback when nothing matches every term.
    pub fn search(&self, query: &str, limit: usize) -> String {
        let terms: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();
        if terms.is_empty() {
            return "empty query".to_string();
        }

        let wiki = self.wiki();
        let mut pages = Vec::new();
        walk_md(&wiki, &mut pages);

        let mut full: Vec<(usize, String)> = Vec::new();
        let mut partial: Vec<(usize, String)> = Vec::new();

        for p in pages {
            let content = match fs::read_to_string(&p) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let lower = content.to_lowercase();
            let counts: Vec<usize> = terms
                .iter()
                .map(|t| lower.matches(t.as_str()).count())
                .collect();
            let score: usize = counts.iter().sum();
            if score == 0 {
                continue;
            }

            let rel = p
                .strip_prefix(&wiki)
                .unwrap_or(&p)
                .display()
                .to_string();
            let title = page_title(&content).unwrap_or_default();

            let mut snips = Vec::new();
            for line in content.lines() {
                let ll = line.to_lowercase();
                if terms.iter().any(|t| ll.contains(t.as_str())) {
                    snips.push(format!("  > {}", clip(line.trim(), 160)));
                    if snips.len() >= 3 {
                        break;
                    }
                }
            }

            let entry = format!("{} - {} (score {})\n{}", rel, title, score, snips.join("\n"));
            if counts.iter().all(|c| *c > 0) {
                full.push((score, entry));
            } else {
                partial.push((score, entry));
            }
        }

        let (mut hits, note) = if full.is_empty() {
            (partial, "(no page matched every term; showing partial matches)\n\n")
        } else {
            (full, "")
        };
        if hits.is_empty() {
            return format!("no matches for: {}", query);
        }
        hits.sort_by(|a, b| b.0.cmp(&a.0));
        let body: Vec<String> = hits.into_iter().take(limit).map(|(_, e)| e).collect();
        format!("{}{}", note, body.join("\n\n"))
    }
}

/// First `# ` heading in a page.
pub fn page_title(content: &str) -> Option<String> {
    content
        .lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l[2..].trim().to_string())
}

pub fn walk_md(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk_md(&p, out);
            } else if p.extension().map_or(false, |x| x == "md") {
                out.push(p);
            }
        }
    }
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// Markdown links to local .md targets.
fn extract_links(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = content[i..].find("](") {
        let start = i + pos + 2;
        match content[start..].find(')') {
            Some(end) => {
                let mut link = &content[start..start + end];
                if let Some(h) = link.find('#') {
                    link = &link[..h];
                }
                if link.ends_with(".md") && !link.starts_with("http") {
                    out.push(link.to_string());
                }
                i = start + end + 1;
            }
            None => break,
        }
    }
    out
}

/// Scaffold a corpus directory. Never overwrites existing files.
pub fn init(c: &Corpus) {
    for d in ["raw", "wiki/incidents", "wiki/patterns", "wiki/systems"] {
        if let Err(e) = fs::create_dir_all(c.root.join(d)) {
            eprintln!("failed to create {}: {}", d, e);
            std::process::exit(1);
        }
    }
    let index = c.wiki().join("index.md");
    if !index.exists() {
        let _ = fs::write(
            &index,
            "# Index\n\nEvery wiki page, one line each.\n\n## Incidents\n\n(none yet)\n\n## Patterns\n\n(none yet)\n\n## Systems\n\n(none yet)\n",
        );
    }
    let log = c.wiki().join("log.md");
    if !log.exists() {
        let _ = fs::write(
            &log,
            "# Log\n\nAppend-only operations record. `+` created, `~` updated, `-` removed.\n",
        );
    }
    println!("corpus ready at {}", c.root.display());
}

/// Broken-link and index-coverage check. Returns true when clean.
pub fn lint(c: &Corpus) -> bool {
    let wiki = c.wiki();
    let mut pages = Vec::new();
    walk_md(&wiki, &mut pages);
    if pages.is_empty() {
        println!("no wiki pages found at {}", wiki.display());
        return false;
    }

    let index = fs::read_to_string(wiki.join("index.md")).unwrap_or_default();
    let mut ok = true;

    for p in &pages {
        let content = fs::read_to_string(p).unwrap_or_default();
        let rel = p
            .strip_prefix(&wiki)
            .unwrap_or(p)
            .display()
            .to_string();
        let dir = p.parent().map(Path::to_path_buf).unwrap_or_else(|| wiki.clone());

        for link in extract_links(&content) {
            if !dir.join(&link).exists() {
                ok = false;
                println!("broken link: {} -> {}", rel, link);
            }
        }

        if rel != "index.md" && rel != "log.md" && !index.contains(&rel) {
            ok = false;
            println!("not in index: {}", rel);
        }
    }

    if ok {
        println!("lint clean: {} pages", pages.len());
    }
    ok
}
