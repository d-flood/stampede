mod common;

use std::{fs, path::Path, process::Command};

use common::{Server, control, next_control};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_tungstenite::connect_async;

#[tokio::test]
async fn changes_are_everything_uncommitted_against_head() {
    let server = Server::start("git").await;
    let repo = server.root.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "trunk"]);
    for name in ["unstaged", "staged", "both", "deleted", "removed", "renamed"] {
        fs::write(repo.join(format!("{name}.txt")), format!("{name}\none\ntwo\nthree\n")).unwrap();
    }
    fs::write(repo.join(".gitignore"), "*.log\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "Initial commit"]);

    fs::write(repo.join("unstaged.txt"), "edited\n").unwrap();
    fs::write(repo.join("staged.txt"), "edited\n").unwrap();
    git(&repo, &["add", "staged.txt"]);
    fs::write(repo.join("both.txt"), "staged\n").unwrap();
    git(&repo, &["add", "both.txt"]);
    fs::write(repo.join("both.txt"), "unstaged\n").unwrap();
    fs::remove_file(repo.join("deleted.txt")).unwrap();
    git(&repo, &["rm", "-q", "removed.txt"]);
    git(&repo, &["mv", "renamed.txt", "moved.txt"]);
    fs::write(repo.join("added.txt"), "added\n").unwrap();
    git(&repo, &["add", "added.txt"]);
    fs::create_dir(repo.join("dir")).unwrap();
    fs::write(repo.join("dir/new file.txt"), "new\n").unwrap();
    fs::write(repo.join("debug.log"), "ignored\n").unwrap();

    let id = open(server.port, &repo).await;
    let expected = json!({
        "isRepo": true,
        "isRoot": true,
        "branch": "trunk",
        "ahead": 0,
        "behind": 0,
        "changes": [
            { "path": "added.txt", "kind": "added" },
            { "path": "both.txt", "kind": "modified" },
            { "path": "deleted.txt", "kind": "deleted" },
            { "path": "dir/new file.txt", "kind": "untracked" },
            { "path": "moved.txt", "kind": "renamed", "from": "renamed.txt" },
            { "path": "removed.txt", "kind": "deleted" },
            { "path": "staged.txt", "kind": "modified" },
            { "path": "unstaged.txt", "kind": "modified" },
        ],
        "staged": [
            { "path": "added.txt", "kind": "added" },
            { "path": "both.txt", "kind": "modified" },
            { "path": "moved.txt", "kind": "renamed", "from": "renamed.txt" },
            { "path": "removed.txt", "kind": "deleted" },
            { "path": "staged.txt", "kind": "modified" },
        ],
        "unstaged": [
            { "path": "both.txt", "kind": "modified" },
            { "path": "deleted.txt", "kind": "deleted" },
            { "path": "dir/new file.txt", "kind": "untracked" },
            { "path": "unstaged.txt", "kind": "modified" },
        ],
        "conflicted": [],
    });
    assert_eq!(get(server.port, &format!("/api/projects/{id}/git")).await, expected);
    assert_eq!(text(server.port, format!("/api/projects/{id}/git/index?path=both.txt")).await, "staged\n");
}

#[tokio::test]
async fn changes_are_staged_unstaged_discarded_committed_and_undone() {
    let server = Server::start("git-actions").await;
    let repo = repository(&server, "actions");
    fs::write(repo.join("kept.txt"), "kept\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "Initial commit"]);
    fs::write(repo.join("kept.txt"), "edited\n").unwrap();
    fs::create_dir(repo.join("dir")).unwrap();
    fs::write(repo.join("dir/new*.txt"), "new\n").unwrap();
    fs::write(repo.join("dir/newer.txt"), "newer\n").unwrap();
    let id = open(server.port, &repo).await;
    let url = format!("/api/projects/{id}/git");
    let status = || get(server.port, &url);

    assert_eq!(post(server.port, &format!("/api/projects/{id}/git/stage"), json!({ "paths": ["dir/new*.txt"] })).await, (204, String::new()));
    let staged = status().await;
    assert_eq!(staged["staged"], json!([{ "path": "dir/new*.txt", "kind": "added" }]));
    assert_eq!(staged["unstaged"], json!([{ "path": "dir/newer.txt", "kind": "untracked" }, { "path": "kept.txt", "kind": "modified" }]));

    assert_eq!(post(server.port, &format!("/api/projects/{id}/git/discard"), json!({ "paths": ["kept.txt", "dir/newer.txt"] })).await.0, 204);
    assert_eq!(fs::read_to_string(repo.join("kept.txt")).unwrap(), "kept\n");
    assert!(!repo.join("dir/newer.txt").exists());
    assert!(repo.join("dir/new*.txt").exists());

    let commit = format!("/api/projects/{id}/git/commit");
    assert_eq!(post(server.port, &commit, json!({ "message": "  " })).await.0, 400);
    assert_eq!(post(server.port, &commit, json!({ "message": "Add new\n\nWith a body" })).await.0, 204);
    assert_eq!(log_message(&repo, "HEAD"), "Add new\n\nWith a body");
    assert_eq!(status().await["staged"], json!([]));
    assert_eq!(post(server.port, &commit, json!({ "message": "Nothing" })).await.0, 409);

    assert_eq!(post(server.port, &format!("/api/projects/{id}/git/undo"), json!(null)).await, (200, json!("Add new\n\nWith a body").to_string()));
    assert_eq!(log_message(&repo, "HEAD"), "Initial commit");
    assert_eq!(status().await["staged"], json!([{ "path": "dir/new*.txt", "kind": "added" }]));

    assert_eq!(post(server.port, &format!("/api/projects/{id}/git/unstage"), json!({ "paths": ["dir/new*.txt"] })).await.0, 204);
    assert_eq!(status().await["unstaged"], json!([{ "path": "dir/new*.txt", "kind": "untracked" }]));
    assert_eq!(post(server.port, &format!("/api/projects/{id}/git/discard"), json!({ "paths": ["../outside"] })).await.0, 403);
}

#[tokio::test]
async fn a_failing_hook_reports_what_it_printed() {
    let server = Server::start("git-hook").await;
    let repo = repository(&server, "hooked");
    fs::write(repo.join("file.txt"), "file\n").unwrap();
    git(&repo, &["add", "."]);
    fs::write(repo.join(".git/hooks/pre-commit"), "#!/bin/sh\necho 'lint: trailing whitespace in file.txt'\nexit 1\n").unwrap();
    Command::new("chmod").arg("+x").arg(repo.join(".git/hooks/pre-commit")).status().unwrap();
    let id = open(server.port, &repo).await;
    let (status, body) = post(server.port, &format!("/api/projects/{id}/git/commit"), json!({ "message": "Add file" })).await;
    assert_eq!((status, body.as_str()), (400, "lint: trailing whitespace in file.txt"));
}

#[tokio::test]
async fn any_unpushed_commit_can_be_reworded_without_touching_the_work() {
    let server = Server::start("git-reword").await;
    let origin = server.root.path().join("origin.git");
    git(server.root.path(), &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
    let repo = repository(&server, "reword");
    git(&repo, &["remote", "add", "origin", origin.to_str().unwrap()]);
    for name in ["one", "two", "three"] {
        fs::write(repo.join(format!("{name}.txt")), format!("{name}\n")).unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", name]);
        if name == "one" { git(&repo, &["push", "-qu", "origin", "main"]); }
    }
    git(&repo, &["checkout", "-qb", "side", "HEAD~1"]);
    fs::write(repo.join("side.txt"), "side\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "side"]);
    git(&repo, &["checkout", "-q", "main"]);
    git(&repo, &["merge", "-q", "--no-ff", "side", "-m", "Merge side"]);
    fs::write(repo.join("three.txt"), "dirty\n").unwrap();
    fs::write(repo.join("one.txt"), "staged\n").unwrap();
    git(&repo, &["add", "one.txt"]);
    let tree = rev_parse(&repo, "HEAD^{tree}");
    let [two, one] = ["HEAD~2", "HEAD~3"].map(|rev| rev_parse(&repo, rev));
    let id = open(server.port, &repo).await;
    let reword = format!("/api/projects/{id}/git/reword");

    assert_eq!(text(server.port, format!("/api/projects/{id}/git/commit/{two}/message")).await, "two");
    assert_eq!(post(server.port, &reword, json!({ "sha": &two[..7], "message": "Two, reworded\n\nWith a body\n" })).await.0, 204);
    assert_eq!(log_message(&repo, "HEAD~2"), "Two, reworded\n\nWith a body");
    assert_eq!(log_message(&repo, "HEAD"), "Merge side");
    assert_eq!(log_message(&repo, "HEAD^2"), "side");
    assert_eq!(rev_parse(&repo, "HEAD^2~1"), rev_parse(&repo, "HEAD~2"));
    assert_eq!(rev_parse(&repo, "HEAD^{tree}"), tree);
    assert_eq!(rev_parse(&repo, "HEAD~3"), one);
    assert_eq!(fs::read_to_string(repo.join("three.txt")).unwrap(), "dirty\n");
    let status = get(server.port, &format!("/api/projects/{id}/git")).await;
    assert_eq!(status["staged"], json!([{ "path": "one.txt", "kind": "modified" }]));
    assert_eq!(status["unstaged"], json!([{ "path": "three.txt", "kind": "modified" }]));

    let head = rev_parse(&repo, "HEAD");
    assert_eq!(post(server.port, &reword, json!({ "sha": head, "message": "Merge the side branch" })).await.0, 204);
    assert_eq!(log_message(&repo, "HEAD"), "Merge the side branch");
    let (status, body) = post(server.port, &reword, json!({ "sha": one, "message": "Pushed" })).await;
    assert_eq!(status, 409);
    assert!(body.contains("origin/main"), "{body}");
    assert_eq!(log_message(&repo, "HEAD~3"), "one");
}

#[tokio::test]
async fn unpushed_commits_squash_into_one_without_touching_the_work() {
    let server = Server::start("git-squash").await;
    let origin = server.root.path().join("origin.git");
    git(server.root.path(), &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
    let repo = repository(&server, "squash");
    git(&repo, &["remote", "add", "origin", origin.to_str().unwrap()]);
    for name in ["one", "two", "three", "four"] {
        fs::write(repo.join(format!("{name}.txt")), format!("{name}\n")).unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["-c", &format!("user.name={name}"), "commit", "-qm", name]);
        if name == "one" { git(&repo, &["push", "-qu", "origin", "main"]); }
    }
    fs::write(repo.join("four.txt"), "dirty\n").unwrap();
    fs::write(repo.join("one.txt"), "staged\n").unwrap();
    git(&repo, &["add", "one.txt"]);
    let [tree, one] = ["HEAD^{tree}", "HEAD~3"].map(|rev| rev_parse(&repo, rev));
    let [head, three, two] = ["HEAD", "HEAD~1", "HEAD~2"].map(|rev| rev_parse(&repo, rev));
    let id = open(server.port, &repo).await;
    let squash = format!("/api/projects/{id}/git/squash");

    let (status, body) = post(server.port, &squash, json!({ "sha": two, "head": three, "message": "Stale" })).await;
    assert_eq!((status, body.as_str()), (409, "The branch has new commits. Look them over and squash again."));
    assert_eq!(post(server.port, &squash, json!({ "sha": head, "head": head, "message": "Alone" })).await.0, 409);
    let (status, body) = post(server.port, &squash, json!({ "sha": one, "head": head, "message": "Pushed" })).await;
    assert_eq!(status, 409);
    assert!(body.contains("origin/main"), "{body}");

    assert_eq!(post(server.port, &squash, json!({ "sha": &two[..7], "head": head, "message": "Two to four\n\nSquashed\n" })).await.0, 204);
    assert_eq!(log_message(&repo, "HEAD"), "Two to four\n\nSquashed");
    assert_eq!(rev_parse(&repo, "HEAD~1"), one);
    assert_eq!(rev_parse(&repo, "HEAD^{tree}"), tree);
    let output = Command::new("git").arg("-C").arg(&repo).args(["log", "-1", "--format=%an"]).output().unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim_end(), "two");
    assert_eq!(fs::read_to_string(repo.join("four.txt")).unwrap(), "dirty\n");
    let status = get(server.port, &format!("/api/projects/{id}/git")).await;
    assert_eq!(status["staged"], json!([{ "path": "one.txt", "kind": "modified" }]));
    assert_eq!(status["unstaged"], json!([{ "path": "four.txt", "kind": "modified" }]));

    git(&repo, &["stash", "-q"]);
    git(&repo, &["checkout", "-qb", "side"]);
    fs::write(repo.join("side.txt"), "side\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "side"]);
    git(&repo, &["checkout", "-q", "main"]);
    git(&repo, &["merge", "-q", "--no-ff", "side", "-m", "Merge side"]);
    let [head, squashed] = ["HEAD", "HEAD~1"].map(|rev| rev_parse(&repo, rev));
    let (status, body) = post(server.port, &squash, json!({ "sha": squashed, "head": head, "message": "Across" })).await;
    assert_eq!((status, body.as_str()), (409, "Merges cannot be squashed."));
}

#[tokio::test]
async fn conflicts_must_be_resolved_before_anything_risky() {
    let server = Server::start("git-conflict").await;
    let repo = repository(&server, "conflict");
    fs::write(repo.join("clash.txt"), "base\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "Base"]);
    git(&repo, &["checkout", "-qb", "other"]);
    fs::write(repo.join("clash.txt"), "theirs\n").unwrap();
    git(&repo, &["commit", "-qam", "Theirs"]);
    git(&repo, &["checkout", "-q", "main"]);
    fs::write(repo.join("clash.txt"), "ours\n").unwrap();
    git(&repo, &["commit", "-qam", "Ours"]);
    let merged = Command::new("git").arg("-C").arg(&repo).args(["merge", "-q", "other"]).output().unwrap();
    assert!(!merged.status.success());
    let id = open(server.port, &repo).await;
    let status = get(server.port, &format!("/api/projects/{id}/git")).await;
    assert_eq!(status["operation"], "merge");
    assert_eq!(status["conflicted"], json!([{ "path": "clash.txt", "kind": "conflicted" }]));
    assert_eq!(status["staged"], json!([]));

    let action = |name: &str| format!("/api/projects/{id}/git/{name}");
    assert_eq!(post(server.port, &action("stage"), json!({ "paths": ["clash.txt"] })).await.0, 409);
    assert_eq!(post(server.port, &action("discard"), json!({ "paths": ["clash.txt"] })).await.0, 409);
    assert_eq!(post(server.port, &action("commit"), json!({ "message": "Merge" })).await.0, 409);
    for name in ["undo", "stashes", "pull"] {
        assert_eq!(post(server.port, &action(name), json!(null)).await.0, 409, "{name}");
    }
    assert_eq!(post(server.port, &action("switch"), json!({ "branch": "other" })).await.0, 409);
    assert!(fs::read_to_string(repo.join("clash.txt")).unwrap().contains("<<<<<<<"));

    fs::write(repo.join("clash.txt"), "both\n").unwrap();
    assert_eq!(post(server.port, &action("stage"), json!({ "paths": ["clash.txt"] })).await.0, 204);
    assert_eq!(post(server.port, &action("commit"), json!({ "message": "Merge other" })).await.0, 204);
    let status = get(server.port, &format!("/api/projects/{id}/git")).await;
    assert!(status.get("operation").is_none());
    assert_eq!(log_message(&repo, "HEAD"), "Merge other");
    assert_eq!(rev_parse(&repo, "HEAD^2"), rev_parse(&repo, "other"));
}

#[tokio::test]
async fn branches_publish_push_fetch_and_fast_forward_against_origin() {
    let server = Server::start("git-remote").await;
    let origin = server.root.path().join("origin.git");
    git(server.root.path(), &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
    let repo = repository(&server, "local");
    git(&repo, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "Initial commit"]);
    let id = open(server.port, &repo).await;
    let url = format!("/api/projects/{id}/git");
    let status = || get(server.port, &url);
    let action = |name: &str| format!("/api/projects/{id}/git/{name}");

    let unpublished = status().await;
    assert_eq!((&unpublished["origin"], unpublished.get("upstream")), (&json!(true), None));
    assert_eq!(post(server.port, &action("push"), json!(null)).await.0, 409);
    assert_eq!(post(server.port, &action("publish"), json!(null)).await.0, 204);
    assert_eq!(status().await["upstream"], "origin/main");

    git(&repo, &["commit", "-q", "--allow-empty", "-m", "Local"]);
    assert_eq!(status().await["ahead"], 1);
    assert_eq!(post(server.port, &action("push"), json!(null)).await.0, 204);
    assert_eq!(status().await["ahead"], 0);

    let other = server.root.path().join("other");
    git(server.root.path(), &["clone", "-q", origin.to_str().unwrap(), other.to_str().unwrap()]);
    git(&other, &["commit", "-q", "--allow-empty", "-m", "Remote"]);
    git(&other, &["push", "-q"]);
    assert_eq!(post(server.port, &action("fetch"), json!(null)).await.0, 204);
    assert_eq!(status().await["behind"], 1);
    assert_eq!(post(server.port, &action("pull"), json!(null)).await.0, 204);
    assert_eq!((status().await["behind"].clone(), log_message(&repo, "HEAD")), (json!(0), "Remote".to_owned()));

    git(&other, &["commit", "-q", "--allow-empty", "-m", "Diverged"]);
    git(&other, &["push", "-q"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "Mine"]);
    assert_eq!(post(server.port, &action("fetch"), json!(null)).await.0, 204);
    assert_eq!(post(server.port, &action("pull"), json!(null)).await.0, 400);
    assert_eq!(log_message(&repo, "HEAD"), "Mine");
    let (code, body) = post(server.port, &action("undo"), json!(null)).await;
    assert_eq!(code, 200, "{body}");
    let (code, body) = post(server.port, &action("undo"), json!(null)).await;
    assert_eq!(code, 409, "{body}");
}

#[tokio::test]
async fn branches_switch_most_recent_first_tracking_remote_ones_when_new() {
    let server = Server::start("git-switch").await;
    let origin = server.root.path().join("origin.git");
    git(server.root.path(), &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
    let repo = repository(&server, "switch");
    git(&repo, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "Initial commit"]);
    git(&repo, &["push", "-q", "origin", "main", "main:shared"]);
    git(&repo, &["checkout", "-qb", "later"]);
    let mut later = Command::new("git");
    later.arg("-C").arg(&repo).args(["commit", "-q", "--allow-empty", "-m", "Later"]).env("GIT_COMMITTER_DATE", "2090-01-01T00:00:00Z");
    assert!(later.status().unwrap().success());
    git(&repo, &["checkout", "-q", "main"]);
    let id = open(server.port, &repo).await;
    let url = format!("/api/projects/{id}/git");
    let status = || get(server.port, &url);
    let switch = |project: &str, branch: &str| {
        let (path, body) = (format!("/api/projects/{project}/git/switch"), json!({ "branch": branch }));
        async move { post(server.port, &path, body).await }
    };

    assert_eq!(get(server.port, &format!("/api/projects/{id}/git/refs")).await["local"][0], "later");
    assert_eq!(switch(&id, "later").await.0, 204);
    assert_eq!(status().await["branch"], "later");
    assert_eq!(switch(&id, "origin/shared").await.0, 204);
    let tracking = status().await;
    assert_eq!((&tracking["branch"], &tracking["upstream"]), (&json!("shared"), &json!("origin/shared")));
    assert_eq!(switch(&id, "main").await.0, 204);
    assert_eq!(switch(&id, "origin/shared").await.0, 204);
    assert_eq!(status().await["branch"], "shared");
    for missing in ["nowhere", "main^", "--detach", "origin/HEAD"] {
        assert_eq!(switch(&id, missing).await.0, 404, "{missing}");
    }

    let (code, body) = post(server.port, &format!("/api/projects/{id}/worktrees"), json!({ "branch": "nested", "from": "main" })).await;
    assert_eq!(code, 200, "{body}");
    let nested = serde_json::from_str::<Value>(&body).unwrap()["projectId"].as_str().unwrap().to_owned();
    let (code, body) = switch(&id, "nested").await;
    assert_eq!(code, 400);
    assert!(body.contains("worktree"), "{body}");
    assert_eq!(status().await["branch"], "shared");
    assert_eq!(switch(&nested, "main").await, (409, "A worktree stays on its own branch.".to_owned()));
}

#[tokio::test]
async fn changes_stash_and_come_back() {
    let server = Server::start("git-stash").await;
    let repo = repository(&server, "stash");
    fs::write(repo.join("file.txt"), "file\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "Initial commit"]);
    fs::write(repo.join("file.txt"), "edited\n").unwrap();
    fs::write(repo.join("new.txt"), "new\n").unwrap();
    let id = open(server.port, &repo).await;
    let stashes = format!("/api/projects/{id}/git/stashes");

    assert_eq!(post(server.port, &stashes, json!(null)).await.0, 204);
    assert_eq!(get(server.port, &format!("/api/projects/{id}/git")).await["changes"], json!([]));
    let listed = get(server.port, &stashes).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert!(listed[0]["message"].as_str().unwrap().starts_with("WIP on main"), "{listed}");
    assert_eq!(post(server.port, &format!("{stashes}/0/peek"), json!(null)).await.0, 404);
    assert_eq!(post(server.port, &format!("{stashes}/0/pop"), json!(null)).await.0, 204);
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "edited\n");
    assert_eq!(fs::read_to_string(repo.join("new.txt")).unwrap(), "new\n");
    assert_eq!(get(server.port, &stashes).await, json!([]));
}

#[tokio::test]
async fn the_commit_rig_drafts_a_message_from_the_staged_diff() {
    let received = tempfile::tempdir().unwrap();
    let log = received.path().join("received.jsonl");
    let fixture = received.path().join("bare.jsonl");
    let turn = fs::read_to_string(format!("{}/tests/fixtures/chats/claude-code/simple-turn.recorded.jsonl", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let bare = [json!(["--tools", ""]), json!(["--setting-sources", ""]), json!(["--strict-mcp-config"]), json!(["--system-prompt", ""])];
    fs::write(&fixture, bare.iter().map(|args| format!("{}\n", json!({ "args": args }))).collect::<String>() + &turn).unwrap();
    let config = format!(r#"
[[rig]]
name = "Coder"
agent = "claude-code"
env = {{ STAMPEDE_FAKE_FIXTURE = "simple-turn" }}

[[rig]]
name = "Writer"
agent = "claude-code"
env = {{ STAMPEDE_FAKE_FIXTURE = "{}", STAMPEDE_FAKE_RECEIVED = "{}" }}
"#, fixture.display(), log.display());
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), std::env::var("PATH").unwrap());
    let server = Server::with_env("git-draft", &config, vec![("PATH", path.into())]).await;
    let repo = repository(&server, "draft");
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "Earlier subject"]);
    fs::write(repo.join("feature.txt"), "a distinctive line\n").unwrap();
    let id = open(server.port, &repo).await;
    let draft = format!("/api/projects/{id}/git/draft");
    assert_eq!(post(server.port, &draft, json!(null)).await.0, 409);
    git(&repo, &["add", "."]);

    let choose = |name: &str| put(server.port, "/api/commit-rig", json!(name));
    assert_eq!(get(server.port, "/api/commit-rig").await, "Coder");
    assert_eq!(choose("Nobody").await.0, 400);
    assert_eq!(choose("Writer").await.0, 200);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while get(server.port, "/api/commit-rig").await != "Writer" { tokio::time::sleep(std::time::Duration::from_millis(20)).await; }
    }).await.unwrap();
    let (code, body) = post(server.port, &draft, json!(null)).await;
    assert_eq!((code, serde_json::from_str::<Value>(&body).unwrap()), (200, json!("Hello and **welcome**! 👋")));
    let asked = || fs::read_to_string(&log).unwrap().lines().map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|message| message["type"] == "user").unwrap()["message"]["content"].as_str().unwrap().to_owned();
    let default = get(server.port, "/api/commit-instructions").await;
    let prompt = asked();
    assert!(prompt.starts_with(default.as_str().unwrap()) && prompt.contains("a distinctive line") && prompt.contains("Earlier subject"), "{prompt}");

    let instructions = |text: &str| put(server.port, "/api/commit-instructions", json!(text));
    let settled = |expected: Value| tokio::time::timeout(std::time::Duration::from_secs(5), async move {
        while get(server.port, "/api/commit-instructions").await != expected { tokio::time::sleep(std::time::Duration::from_millis(20)).await; }
    });
    assert_eq!(instructions("  Write a haiku.\nNo body.\n").await, (200, json!("Write a haiku.\nNo body.").to_string()));
    settled(json!("Write a haiku.\nNo body.")).await.unwrap();
    fs::remove_file(&log).unwrap();
    assert_eq!(post(server.port, &draft, json!(null)).await.0, 200);
    assert!(asked().starts_with("Write a haiku.\nNo body.\n\n"));
    assert_eq!(instructions(" ").await, (200, default.to_string()));
    settled(default).await.unwrap();
    assert!(!fs::read_to_string(server.root.path().join("config.toml")).unwrap().contains("commit_instructions"));
}

#[tokio::test]
async fn a_detached_head_is_named_by_its_short_commit() {
    let server = Server::start("git").await;
    let repo = server.root.path().join("detached");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "Initial commit"]);
    git(&repo, &["checkout", "-q", "--detach"]);

    let id = open(server.port, &repo).await;
    let status = get(server.port, &format!("/api/projects/{id}/git")).await;
    assert_eq!(status["branch"], rev_parse(&repo, "HEAD")[..7]);
}

#[tokio::test]
async fn a_non_git_directory_is_not_a_repo() {
    let server = Server::start("git").await;
    let dir = server.root.path().join("plain");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("file.txt"), "plain\n").unwrap();

    let id = open(server.port, &dir).await;
    let expected = json!({ "isRepo": false, "isRoot": false, "ahead": 0, "behind": 0, "changes": [], "staged": [], "unstaged": [], "conflicted": [] });
    assert_eq!(get(server.port, &format!("/api/projects/{id}/git")).await, expected);
    assert_eq!(get(server.port, &format!("/api/projects/{id}/git/log")).await, json!({ "commits": [] }));
}

#[tokio::test]
async fn a_commit_is_diffed_against_its_first_parent() {
    let server = Server::start("git").await;
    let repo = server.root.path().join("history");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::write(repo.join("a.txt"), "one\n").unwrap();
    fs::write(repo.join("b.txt"), "b\nb\nb\n").unwrap();
    fs::write(repo.join("gone.txt"), "gone\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "Root"]);
    git(&repo, &["checkout", "-qb", "side"]);
    fs::write(repo.join("side.txt"), "side\n").unwrap();
    git(&repo, &["add", "side.txt"]);
    git(&repo, &["commit", "-qm", "Side"]);
    git(&repo, &["checkout", "-q", "main"]);
    fs::write(repo.join("a.txt"), "two\n").unwrap();
    git(&repo, &["mv", "b.txt", "c.txt"]);
    git(&repo, &["rm", "-q", "gone.txt"]);
    git(&repo, &["commit", "-qam", "Main"]);
    git(&repo, &["merge", "-q", "--no-ff", "side", "-m", "Merge side"]);
    let [merge, main, side, root] = ["HEAD", "HEAD^", "HEAD^2", "HEAD~2"].map(|rev| rev_parse(&repo, rev));

    let id = open(server.port, &repo).await;
    let log = get(server.port, &format!("/api/projects/{id}/git/log")).await;
    let commits = log["commits"].as_array().unwrap();
    let shas: Vec<_> = commits.iter().map(|commit| commit["sha"].as_str().unwrap()).collect();
    assert_eq!(shas, [&merge, &main, &side, &root].map(String::as_str));
    assert_eq!((&commits[0]["subject"], &commits[0]["author"]), (&json!("Merge side"), &json!("stampede")));
    assert!(merge.starts_with(commits[0]["short"].as_str().unwrap()));
    assert!(log.get("cursor").is_none());

    let files = |sha: &str| format!("/api/projects/{id}/git/commit/{sha}");
    let expected = json!([{ "path": "side.txt", "kind": "added" }]);
    assert_eq!(get(server.port, &files(&merge)).await, expected);
    let expected = json!([
        { "path": "a.txt", "kind": "modified" },
        { "path": "c.txt", "kind": "renamed", "from": "b.txt" },
        { "path": "gone.txt", "kind": "deleted" },
    ]);
    assert_eq!(get(server.port, &files(&main)).await, expected);
    let expected = json!([
        { "path": "a.txt", "kind": "added" },
        { "path": "b.txt", "kind": "added" },
        { "path": "gone.txt", "kind": "added" },
    ]);
    assert_eq!(get(server.port, &files(&root)).await, expected);

    let port = server.port;
    assert_eq!(sides(port, &id, &merge, "side.txt", "side.txt").await, ["", "side\n"]);
    assert_eq!(sides(port, &id, &merge, "a.txt", "a.txt").await, ["two\n", "two\n"]);
    assert_eq!(sides(port, &id, &main, "b.txt", "c.txt").await, ["b\nb\nb\n", "b\nb\nb\n"]);
    assert_eq!(sides(port, &id, &main, "gone.txt", "gone.txt").await, ["gone\n", ""]);
    assert_eq!(sides(port, &id, &root, "a.txt", "a.txt").await, ["", "one\n"]);
}

fn repository(server: &Server, name: &str) -> std::path::PathBuf {
    let repo = server.root.path().join(name);
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    for (key, value) in [("user.name", "stampede"), ("user.email", "test@stampede.invalid"), ("commit.gpgsign", "false")] {
        git(&repo, &["config", key, value]);
    }
    repo
}

fn log_message(repo: &Path, rev: &str) -> String {
    let output = Command::new("git").arg("-C").arg(repo).args(["log", "-1", "--format=%B", rev]).output().unwrap();
    String::from_utf8(output.stdout).unwrap().trim_end().to_owned()
}

async fn post(port: u16, path: &str, body: Value) -> (u16, String) {
    send(reqwest::Client::new().post(format!("http://127.0.0.1:{port}{path}")), body).await
}

async fn put(port: u16, path: &str, body: Value) -> (u16, String) {
    send(reqwest::Client::new().put(format!("http://127.0.0.1:{port}{path}")), body).await
}

async fn send(request: reqwest::RequestBuilder, body: Value) -> (u16, String) {
    let request = if body.is_null() { request } else { request.header("Content-Type", "application/json").body(body.to_string()) };
    let response = request.send().await.unwrap();
    (response.status().as_u16(), response.text().await.unwrap())
}

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=stampede", "-c", "user.email=test@stampede.invalid", "-C"])
        .arg(repo)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

async fn open(port: u16, dir: &Path) -> String {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": dir })).await;
    next_control(&mut socket, "project.opened").await["projectId"].as_str().unwrap().to_owned()
}

fn rev_parse(repo: &Path, rev: &str) -> String {
    let output = Command::new("git").arg("-C").arg(repo).args(["rev-parse", rev]).output().unwrap();
    assert!(output.status.success(), "git rev-parse {rev}");
    String::from_utf8(output.stdout).unwrap().trim_end().to_owned()
}

async fn sides(port: u16, id: &str, sha: &str, from: &str, path: &str) -> [String; 2] {
    let show = |rev: &str, path: &str| text(port, format!("/api/projects/{id}/git/show?sha={rev}&path={path}"));
    [show(&format!("{sha}%5E"), from).await, show(sha, path).await]
}

async fn get(port: u16, path: &str) -> Value {
    serde_json::from_str(&text(port, path.to_owned()).await).unwrap()
}

async fn text(port: u16, path: String) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    body.to_owned()
}
