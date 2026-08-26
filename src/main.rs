//! Repo-wide language breakdown by byte count, like GitHub's "Languages" bar.
//!
//! Usage: langlist [--json] [root-dir]
//!
//! ```text
//! $ langlist ~Dev/Clones/linguist # https://github.com/drshade/linguist
//! Rust                  97.0%  (60023 bytes)
//! Shell                  3.0%  (1869 bytes)
//! ```

use linguist::{detect_language_by_extension, disambiguate, is_vendored};
use linguist_types::LanguageType;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(serde::Serialize)]
struct LanguageStat {
    language: &'static str,
    bytes: u64,
    percent: f64,
}

fn main() {
    let mut root = ".".to_string();
    let mut json = false;
    for arg in std::env::args().skip(1) {
        // No clap-rs/clap for this, until complexity demands it.
        if arg == "--json" {
            json = true;
        } else {
            root = arg;
        }
    }

    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(&root)
        .output()
        .expect("git ls-files failed - is this a git repo?");
    let files: Vec<PathBuf> = output
        .stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| Path::new(&root).join(String::from_utf8_lossy(s).as_ref()))
        .collect();

    let sized: Vec<(&'static str, u64)> = files
        .par_iter()
        .filter_map(|path| {
            let path_str = path.to_string_lossy();
            if is_vendored(path_str.as_ref()).unwrap_or(false) {
                return None;
            }

            let candidates =
                detect_language_by_extension(path_str.as_ref()).ok()?;

            let resolved = if candidates.len() == 1 {
                candidates.into_iter().next()
            } else if candidates.is_empty() {
                None
            } else {
                let content = fs::read_to_string(path).ok()?;
                disambiguate(path_str.as_ref(), &content)
                    .ok()
                    .and_then(|v| v.into_iter().next())
                    .or_else(|| candidates.into_iter().next())
            };

            let language = resolved?;
            if language.definition.language_type != LanguageType::Programming {
                return None;
            }

            let size = fs::metadata(path).map(|m| m.len()).ok()?;
            Some((language.name, size))
        })
        .collect();

    let mut bytes_by_language: HashMap<&'static str, u64> = HashMap::new();
    for (name, size) in sized {
        *bytes_by_language.entry(name).or_insert(0) += size;
    }

    let total: u64 = bytes_by_language.values().sum();
    if total == 0 {
        if json {
            // Return valid JSON for empty results.
            println!("[]");
        } else {
            println!("No programming-language files found under {root}");
        }
        return;
    }

    let mut ranked: Vec<_> = bytes_by_language.into_iter().collect();
    ranked.sort_by_key(|&(_, size)| std::cmp::Reverse(size));

    if json {
        let stats: Vec<LanguageStat> = ranked
            .into_iter()
            .map(|(language, bytes)| LanguageStat {
                language,
                bytes,
                percent: (bytes as f64 / total as f64 * 1000.0).round() / 10.0,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&stats).unwrap());
    } else {
        for (name, size) in ranked {
            let pct = size as f64 / total as f64 * 100.0;
            println!("{name:<20} {pct:5.1}%  ({size} bytes)");
        }
    }
}
