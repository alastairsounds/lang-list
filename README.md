# langlist
Repo-wide language breakdown by byte count, like GitHub's "Languages" bar.

## Usage
```text
langlist [--json] [--repo owner/repo|url] [root-dir]
```

Defaults to the current directory. Requires the target to be a git repo (uses `git ls-files` to find tracked files). Pass `--json` for machine-readable output.

```text
$ langlist ~/Dev/Clones/linguist
Rust                  97.0%  (60023 bytes)
Shell                  3.0%  (1869 bytes)
```

### GitHub repos
Pass a GitHub `owner/repo` slug or URL (via `--repo`, or as the bare positional
argument if it isn't an existing local path) to fetch that repo's language
breakdown straight from GitHub's API, no local clone needed:

```text
$ langlist --repo drshade/linguist
Rust                  97.7%  (66143 bytes)
Shell                  2.3%  (1549 bytes)
```

Set `GITHUB_TOKEN` for higher API rate limits (optional, public repos work
without it).

## Screenshot
![screenshot](_demo/langlist.png)

## Build
```shell
cargo build --release
```
