//! Repo-wide language breakdown by byte count, like GitHub's "Languages" bar.
//!
//! Usage: langlist [--json] [--repo owner/repo|url] [root-dir]
//!
//! ```text
//! $ langlist drshade/linguist      # fetches straight from GitHub, no clone needed
//! $ langlist ~Dev/Clones/linguist  # scans a local clone of the same repo instead
//! Rust                  97.0%  (60023 bytes)
//! Shell                  3.0%  (1869 bytes)
//! ```

use clap::Parser;
use langlist::{github, LanguageStat};
use std::path::Path;

/// Byte-per-language breakdown of a local directory or a GitHub repo.
#[derive(Parser)]
struct Cli {
    // arg #1: empty -> current directory; local/dir -> local directory; owner/repo -> GitHub repo
    /// Local directory to scan (defaults to the current directory)
    root: Option<String>,

    /// Emit machine-readable JSON instead of the text table
    #[arg(long)]
    json: bool,

    /// GitHub owner/repo slug or URL, e.g. torvalds/linux
    #[arg(long)]
    repo: Option<String>,
}

enum Target {
    Local(String),
    Remote(String, String),
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    let json = cli.json;

    match resolve_target(cli.root, cli.repo) {
        Target::Local(dir) => {
            let stats = langlist::scan_directory(&dir);
            let empty_message = format!("No language files found under {dir}");
            print_stats(&stats, json, &empty_message);
        }
        Target::Remote(owner, repo) => {
            let stats = github::fetch_repo_languages(&owner, &repo)
                .await
                .unwrap_or_else(|e| {
                    eprintln!("{e}");
                    std::process::exit(1);
                });
            print_stats(
                &stats,
                json,
                &format!("No language data returned for {owner}/{repo}"),
            );
        }
    }
}

fn resolve_target(root: Option<String>, repo_ref: Option<String>) -> Target {
    if let Some(reference) = repo_ref {
        return match github::parse_repo_ref(&reference) {
            Ok((owner, repo)) => Target::Remote(owner, repo),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        };
    }

    let arg = root.unwrap_or_else(|| ".".to_string());
    if Path::new(&arg).exists() {
        return Target::Local(arg);
    }

    match github::parse_repo_ref(&arg) {
        Ok((owner, repo)) => Target::Remote(owner, repo),
        Err(_) => {
            eprintln!("not a local directory or a valid GitHub repo: {arg}");
            std::process::exit(1);
        }
    }
}

fn print_stats(stats: &[LanguageStat], json: bool, empty_message: &str) {
    if stats.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("{empty_message}");
        }
        return;
    }

    if json {
        println!("{}", serde_json::to_string_pretty(stats).unwrap());
    } else {
        for s in stats {
            let name = &s.language;
            println!("{name:<20} {:5.1}%  ({} bytes)", s.percent, s.bytes);
        }
    }
}
