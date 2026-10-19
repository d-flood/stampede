use std::path::Path;

use crate::git;

/// The submodules checked out in `dir`, by path.
pub async fn checked_out(dir: &Path) -> Result<Vec<String>, String> {
    let paths = git::run(dir, &["submodule", "foreach", "--quiet", "echo \"$sm_path\""]).await?;
    Ok(paths.lines().map(str::to_owned).collect())
}

/// Checks out in a new worktree the submodules `source` has checked out, at the commits the worktree records. A commit
/// their remote lacks, such as one an earlier Ticket made, is fetched from `source`'s clone.
pub async fn init(worktree: &Path, source: &Path) -> Result<(), String> {
    let paths = checked_out(source).await?;
    if paths.is_empty() {
        return Ok(());
    }
    let mut update = vec!["submodule", "update", "--init", "--"];
    update.extend(paths.iter().map(String::as_str));
    if git::run(worktree, &update).await.is_ok() {
        return Ok(());
    }
    let mut tree = vec!["ls-tree", "-z", "HEAD", "--"];
    tree.extend(paths.iter().map(String::as_str));
    for entry in git::run(worktree, &tree).await?.split('\0') {
        let Some((meta, path)) = entry.split_once('\t') else { continue };
        if let ["160000", _, commit] = meta.split(' ').collect::<Vec<_>>()[..]
            && worktree.join(path).join(".git").exists()
        {
            let _ = fetch(&source.join(path), &worktree.join(path), commit).await;
        }
    }
    update.insert(3, "--no-fetch");
    git::run(worktree, &update).await.map(drop)
}

/// Fetches into `target`'s submodules, from `source`'s, each submodule commit that merging `to` into `from` brings in,
/// and gives their paths. Every worktree has its own clone of each submodule, and a merge moves only the commit recorded.
pub async fn carry(source: &Path, target: &Path, from: &str, to: &str) -> Result<Vec<String>, String> {
    let range = format!("{from}...{to}");
    let diff = git::run(target, &["diff", "--raw", "-z", "--no-abbrev", "--no-renames", "--ignore-submodules=none", &range]).await?;
    let mut fields = diff.split('\0');
    let mut moved = Vec::new();
    while let (Some(header), Some(path)) = (fields.next(), fields.next()) {
        let [_, "160000", _, commit, status] = header.split(' ').collect::<Vec<_>>()[..] else { continue };
        if status == "D" {
            continue;
        }
        let into = target.join(path);
        if !into.join(".git").exists() {
            return Err(format!("the submodule {path} is not checked out in {}, so its commit {commit} could not be fetched there", target.display()));
        }
        fetch(&source.join(path), &into, commit)
            .await
            .map_err(|error| format!("the commit {commit} of the submodule {path} could not be fetched from {}: {error}", source.display()))?;
        moved.push(path.to_owned());
    }
    Ok(moved)
}

/// Checks out in `dir` the commits its index records for the submodules at `paths`, which a merge left behind.
pub async fn sync(dir: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut update = vec!["submodule", "update", "--no-fetch", "--"];
    update.extend(paths.iter().map(String::as_str));
    git::run(dir, &update).await.map(drop)
}

/// What is uncommitted in `dir` and in each submodule checked out there; empty when nothing is.
pub async fn status(dir: &Path) -> Result<String, String> {
    let status = git::run(dir, &["status", "--porcelain", "--ignore-submodules=none"]).await?;
    let inside = git::run(dir, &["submodule", "foreach", "--quiet", "git status --porcelain"]).await?;
    Ok(status + &inside)
}

async fn fetch(source: &Path, target: &Path, commit: &str) -> Result<(), String> {
    if git::run(target, &["cat-file", "-e", &format!("{commit}^{{commit}}")]).await.is_ok() {
        return Ok(());
    }
    git::run(target, &["fetch", "-q", "--no-tags", &source.to_string_lossy(), commit]).await.map(drop)
}
