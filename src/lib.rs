//! Core language-breakdown logic used by the CLI.

use serde::Serialize;

#[derive(Serialize)]
pub struct LanguageStat {
    // Provided either by drshade/linguist (local) or GitHub API (remote)
    pub language: String,
    pub bytes: u64,
    pub percent: f64,
    pub color: Option<String>,
    pub extensions: Option<Vec<String>>,
    pub filenames: Option<Vec<String>>,
}

type LanguageEntry = (String, u64, Option<&'static linguist_types::Language>);

/// Sorts by bytes descending, computes percentages, and builds `LanguageStat`s.
fn finalize(mut entries: Vec<LanguageEntry>) -> Vec<LanguageStat> {
    let total: u64 = entries.iter().map(|&(_, bytes, _)| bytes).sum();
    if total == 0 {
        return Vec::new();
    }

    entries.sort_by_key(|&(_, bytes, _)| std::cmp::Reverse(bytes));
    entries
        .into_iter()
        .map(|(language, bytes, definition)| LanguageStat {
            language,
            bytes,
            percent: (bytes as f64 / total as f64 * 1000.0).round() / 10.0,
            color: definition.and_then(|d| d.color.clone()),
            extensions: definition.and_then(|d| d.extensions.clone()),
            filenames: definition.and_then(|d| d.filenames.clone()),
        })
        .collect()
}

/// Byte-per-language breakdown of a local git-tracked directory.
pub fn scan_directory(root: &str) -> Vec<LanguageStat> {
    use linguist::{detect_language_by_extension, disambiguate, is_vendored};
    use linguist_types::LanguageType;
    use rayon::prelude::*;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .expect("git ls-files failed - is this a git repo?");
    let files: Vec<PathBuf> = output
        .stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| Path::new(root).join(String::from_utf8_lossy(s).as_ref()))
        .collect();

    type Detected = (&'static str, u64, &'static linguist_types::Language);
    let sized: Vec<Detected> = files
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
            Some((language.name, size, language.definition))
        })
        .collect();

    type WithDef = (u64, &'static linguist_types::Language);
    let mut bytes_by_language: HashMap<&'static str, WithDef> = HashMap::new();
    for (name, size, definition) in sized {
        bytes_by_language.entry(name).or_insert((0, definition)).0 += size;
    }

    let entries = bytes_by_language
        .into_iter()
        .map(|(name, (bytes, def))| (name.to_string(), bytes, Some(def)))
        .collect();
    finalize(entries)
}

pub mod github {
    use super::{finalize, LanguageStat};
    use std::collections::HashMap;
    use std::fmt;

    #[derive(Debug)]
    pub struct ParseError(pub String);

    impl fmt::Display for ParseError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for ParseError {}

    #[derive(Debug)]
    pub struct GithubError(pub String);

    impl fmt::Display for GithubError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for GithubError {}

    /// Parses an `owner/repo` slug or a `github.com` URL (with an optional
    /// `.git` suffix or trailing path) into `(owner, repo)`.
    pub fn parse_repo_ref(input: &str) -> Result<(String, String), ParseError> {
        let trimmed = input.trim();
        let after_host = trimmed
            .strip_prefix("https://github.com/")
            .or_else(|| trimmed.strip_prefix("http://github.com/"))
            .or_else(|| trimmed.strip_prefix("https://www.github.com/"))
            .or_else(|| trimmed.strip_prefix("http://www.github.com/"))
            .or_else(|| trimmed.strip_prefix("github.com/"))
            .unwrap_or(trimmed);

        let mut parts = after_host.splitn(3, '/');
        let owner = parts.next().unwrap_or("");
        let repo = parts.next().unwrap_or("");
        let repo = repo.strip_suffix(".git").unwrap_or(repo);

        if is_valid_segment(owner) && is_valid_segment(repo) {
            Ok((owner.to_string(), repo.to_string()))
        } else {
            Err(ParseError(format!(
                "not a valid GitHub repo reference: {input}"
            )))
        }
    }

    /// A GitHub owner/repo path segment: non-empty, URL-safe, no `/`.
    fn is_valid_segment(s: &str) -> bool {
        let safe = |c: char| c.is_ascii_alphanumeric() || "-_.".contains(c);
        !s.is_empty() && s.chars().all(safe)
    }

    /// Fetches a repo's language byte-counts from GitHub's REST API.
    pub async fn fetch_repo_languages(
        owner: &str,
        repo: &str,
    ) -> Result<Vec<LanguageStat>, GithubError> {
        let url =
            format!("https://api.github.com/repos/{owner}/{repo}/languages");
        let mut request = reqwest::Client::new()
            .get(&url)
            .header("User-Agent", "langlist")
            .header("Accept", "application/vnd.github+json");
        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            let auth = format!("Bearer {token}");
            request = request.header("Authorization", auth);
        }

        let response = request
            .send()
            .await
            .map_err(|e| GithubError(e.to_string()))?;
        if !response.status().is_success() {
            return Err(GithubError(format!(
                "GitHub API returned {} for {owner}/{repo}",
                response.status()
            )));
        }

        let languages: HashMap<String, u64> = response
            .json()
            .await
            .map_err(|e| GithubError(e.to_string()))?;

        let entries = languages
            .into_iter()
            .map(|(name, bytes)| {
                let definition = linguist::definitions::LANGUAGES.get(&name);
                (name, bytes, definition)
            })
            .collect();
        Ok(finalize(entries))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_slug() {
            assert_eq!(
                parse_repo_ref("torvalds/linux").unwrap(),
                ("torvalds".to_string(), "linux".to_string())
            );
        }

        #[test]
        fn parses_full_url() {
            let url = "https://github.com/torvalds/linux";
            assert_eq!(
                parse_repo_ref(url).unwrap(),
                ("torvalds".to_string(), "linux".to_string())
            );
        }

        #[test]
        fn parses_full_url_www_subdomain() {
            let url = "https://www.github.com/torvalds/linux";
            assert_eq!(
                parse_repo_ref(url).unwrap(),
                ("torvalds".to_string(), "linux".to_string())
            );
        }

        #[test]
        fn strips_git_suffix() {
            let url = "https://github.com/torvalds/linux.git";
            assert_eq!(
                parse_repo_ref(url).unwrap(),
                ("torvalds".to_string(), "linux".to_string())
            );
        }

        #[test]
        fn strips_trailing_path() {
            let url = "https://github.com/torvalds/linux/tree/master";
            assert_eq!(
                parse_repo_ref(url).unwrap(),
                ("torvalds".to_string(), "linux".to_string())
            );
        }

        #[test]
        fn rejects_invalid_input() {
            assert!(parse_repo_ref("not-a-repo-ref").is_err());
            assert!(parse_repo_ref("").is_err());
            assert!(parse_repo_ref("owner/repo?x=1").is_err());
        }
    }
}
