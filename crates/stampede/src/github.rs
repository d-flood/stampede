use std::{io::ErrorKind, path::Path, process::Stdio, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{io::AsyncWriteExt, process::Command, time::timeout};

use crate::git;

const LIMIT: Duration = Duration::from_secs(30);
const GITHUB: &str = "github.com";

/// The GitHub repository `gh` resolves in a Project, and the Project's remote that fetches from it.
#[derive(Clone, Debug)]
pub struct Repository {
    pub host: String,
    pub owner: String,
    pub name: String,
    pub remote: String,
}

pub enum Availability {
    Available(Repository),
    Unavailable(String),
    Hidden,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pr {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub is_draft: bool,
    pub requested: bool,
    pub updated_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pull {
    pub title: String,
    pub url: String,
    pub author: Author,
    pub state: String,
    pub base_ref_name: String,
}

#[derive(Deserialize)]
pub struct Author {
    pub login: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Listed {
    number: u64,
    title: String,
    author: Author,
    is_draft: bool,
    updated_at: String,
}

enum Failure {
    Missing,
    Failed(String),
}

impl From<Failure> for String {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::Missing => "gh is not installed".to_owned(),
            Failure::Failed(message) => message,
        }
    }
}

async fn gh(dir: &Path, args: &[&str]) -> Result<Vec<u8>, Failure> {
    gh_with(dir, args, None).await
}

async fn gh_with(dir: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>, Failure> {
    let mut command = Command::new("gh");
    command
        .current_dir(dir)
        .args(args)
        .envs([("GH_PROMPT_DISABLED", "1"), ("GH_NO_UPDATE_NOTIFIER", "1")])
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let run = async {
        let mut child = command.spawn()?;
        if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
            // Ignore a broken pipe: a gh that exits early says why on stderr.
            let _ = stdin.write_all(input).await;
        }
        child.wait_with_output().await
    };
    let output = match timeout(LIMIT, run).await {
        Err(_) => return Err(Failure::Failed(format!("gh {} timed out", args.join(" ")))),
        Ok(Err(error)) if error.kind() == ErrorKind::NotFound => return Err(Failure::Missing),
        Ok(Err(error)) => return Err(Failure::Failed(format!("cannot run gh: {error}"))),
        Ok(Ok(output)) => output,
    };
    if output.status.success() {
        return Ok(output.stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(Failure::Failed(if stderr.is_empty() { format!("gh {} failed", args.join(" ")) } else { stderr }))
}

fn parsed<T: for<'de> Deserialize<'de>>(output: &[u8]) -> Result<T, String> {
    serde_json::from_slice(output).map_err(|error| format!("cannot read gh's output: {error}"))
}

/// The host, owner and name an https, ssh or scp-style GitHub URL names.
pub fn parse_url(url: &str) -> Option<(String, String, String)> {
    let rest = match url.split_once("://") {
        Some((_, rest)) => rest,
        None => &url.replacen(':', "/", 1),
    };
    let (authority, path) = rest.split_once('/')?;
    let host = authority.rsplit('@').next()?.split(':').next()?.to_ascii_lowercase();
    let mut parts = path.trim_end_matches('/').split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    let name = name.strip_suffix(".git").unwrap_or(name);
    (!host.is_empty() && !owner.is_empty() && !name.is_empty()).then(|| (host, owner.to_owned(), name.to_owned()))
}

/// Remotes by name and their raw URLs, before any `insteadOf` rewriting.
async fn remotes(dir: &Path) -> Vec<(String, String)> {
    let listed = git::run(dir, &["config", "--get-regexp", r"^remote\..*\.url$"]).await.unwrap_or_default();
    listed
        .lines()
        .filter_map(|line| {
            let (key, url) = line.split_once(' ')?;
            Some((key.strip_prefix("remote.")?.strip_suffix(".url")?.to_owned(), url.to_owned()))
        })
        .collect()
}

fn hostname(host: &str) -> Vec<&str> {
    if host == GITHUB { Vec::new() } else { vec!["--hostname", host] }
}

pub async fn resolve(dir: &Path) -> Availability {
    let remotes: Vec<_> = remotes(dir).await.into_iter().filter_map(|(remote, url)| Some((remote, parse_url(&url)?))).collect();
    if remotes.is_empty() {
        return Availability::Hidden;
    }
    let viewed = match gh(dir, &["repo", "view", "--json", "nameWithOwner,url"]).await {
        Err(Failure::Missing) => return Availability::Unavailable("Install the GitHub CLI (gh) to review pull requests".to_owned()),
        Err(Failure::Failed(_)) => None,
        Ok(output) => {
            let value: Value = parsed(&output).unwrap_or_default();
            value["url"].as_str().and_then(parse_url)
        }
    };
    let Some((host, owner, name)) = viewed else {
        for (_, (host, ..)) in &remotes {
            if gh(dir, &["auth", "status", "--hostname", host]).await.is_ok() {
                return Availability::Hidden;
            }
        }
        return match remotes.iter().any(|(_, (host, ..))| host == GITHUB) {
            true => Availability::Unavailable(signed_out(GITHUB)),
            false => Availability::Hidden,
        };
    };
    let same = |(other_host, other_owner, other_name): &(String, String, String)| {
        *other_host == host && other_owner.eq_ignore_ascii_case(&owner) && other_name.eq_ignore_ascii_case(&name)
    };
    let Some((remote, _)) = remotes.iter().find(|(_, parts)| same(parts)) else { return Availability::Hidden };
    if gh(dir, &["auth", "status", "--hostname", &host]).await.is_err() {
        return Availability::Unavailable(signed_out(&host));
    }
    Availability::Available(Repository { host, owner, name, remote: remote.clone() })
}

fn signed_out(host: &str) -> String {
    format!("gh is not signed in to {host}; run gh auth login --hostname {host}")
}

/// Open pull requests, those requesting the signed-in user's review first, then the most recently updated.
pub async fn prs(dir: &Path) -> Result<Vec<Pr>, String> {
    let fields = "number,title,author,isDraft,updatedAt";
    let all = ["pr", "list", "--state", "open", "--limit", "1000", "--json", fields];
    let mine = ["pr", "list", "--state", "open", "--limit", "1000", "--search", "review-requested:@me", "--json", fields];
    let (listed, requested) = tokio::join!(gh(dir, &all), gh(dir, &mine));
    let listed: Vec<Listed> = parsed(&listed?)?;
    let requested: Vec<Listed> = parsed(&requested?)?;
    let numbers: Vec<u64> = requested.iter().map(|pr| pr.number).collect();
    let others = listed.into_iter().filter(|pr| !numbers.contains(&pr.number));
    let mut prs: Vec<Pr> = requested
        .into_iter()
        .map(|pr| (true, pr))
        .chain(others.map(|pr| (false, pr)))
        .map(|(requested, pr)| Pr {
            requested,
            number: pr.number,
            title: pr.title,
            author: pr.author.login,
            is_draft: pr.is_draft,
            updated_at: pr.updated_at,
        })
        .collect();
    prs.sort_by(|a, b| b.requested.cmp(&a.requested).then_with(|| b.updated_at.cmp(&a.updated_at)));
    Ok(prs)
}

pub enum Lookup {
    Unknown(String),
    Failed(String),
}

pub async fn pull(dir: &Path, number: u64) -> Result<Pull, Lookup> {
    let number = number.to_string();
    match gh(dir, &["pr", "view", &number, "--json", "number,title,url,author,state,baseRefName"]).await {
        Ok(output) => parsed(&output).map_err(Lookup::Failed),
        Err(Failure::Failed(message)) if message.contains("Could not resolve") || message.contains("no pull requests found") => {
            Err(Lookup::Unknown(message))
        }
        Err(failure) => Err(Lookup::Failed(failure.into())),
    }
}

pub async fn login(dir: &Path, host: &str) -> Result<String, String> {
    let args = [&["api"][..], &hostname(host), &["user"]].concat();
    let user: Value = parsed(&gh(dir, &args).await?)?;
    user["login"].as_str().map(str::to_owned).ok_or_else(|| "gh api user named no login".to_owned())
}

/// Posts `review` to pull request `number` as one review, giving GitHub's message when it refuses.
pub async fn send_review(dir: &Path, repository: &Repository, number: u64, review: &Value) -> Result<(), String> {
    let path = format!("repos/{}/{}/pulls/{number}/reviews", repository.owner, repository.name);
    let args = [&["api"][..], &hostname(&repository.host), &["--method", "POST", &path, "--input", "-"]].concat();
    gh_with(dir, &args, Some(review.to_string().as_bytes())).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_name_host_owner_and_repository_in_every_form() {
        let widgets = Some(("github.com".to_owned(), "acme".to_owned(), "widgets".to_owned()));
        for url in [
            "https://github.com/acme/widgets.git",
            "https://github.com/acme/widgets",
            "https://user@GitHub.com/acme/widgets/",
            "git@github.com:acme/widgets.git",
            "ssh://git@github.com:22/acme/widgets.git",
        ] {
            assert_eq!(parse_url(url), widgets, "{url}");
        }
        assert_eq!(parse_url("git@ghe.corp:team/tool"), Some(("ghe.corp".to_owned(), "team".to_owned(), "tool".to_owned())));
        assert_eq!(parse_url("/srv/git/widgets.git"), None);
    }
}
