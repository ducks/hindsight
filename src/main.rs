mod corpus;
mod mcp;

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

Corpus resolution order:
  --data flag, $HINDSIGHT_DATA, $XDG_DATA_HOME/hindsight,
  ~/.local/share/hindsight";

fn main() {
    let mut data: Option<PathBuf> = None;
    let mut cmd: Option<String> = None;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data" => data = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                println!("{}", USAGE);
                return;
            }
            other => {
                if cmd.is_none() {
                    cmd = Some(other.to_string());
                }
            }
        }
    }

    let corpus = Corpus::resolve(data);

    match cmd.as_deref() {
        Some("serve") => mcp::serve(&corpus),
        Some("init") => corpus::init(&corpus),
        Some("lint") => {
            if !corpus::lint(&corpus) {
                std::process::exit(1);
            }
        }
        Some("path") => println!("{}", corpus.root.display()),
        _ => {
            eprintln!("{}", USAGE);
            std::process::exit(2);
        }
    }
}
