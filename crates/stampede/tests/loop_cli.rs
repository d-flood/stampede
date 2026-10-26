use crate::common;

use std::{fs, path::Path};

use common::{Cli, Server, git, init};
use serde_json::Value;

#[tokio::test]
async fn ticket_tags_are_optional_overlapping_validated_and_persistent() {
    let mut server = Server::start("ticket-tags").await;
    let root = server.root.path().to_owned();
    let cli = Cli { data_dir: root.join("data") };
    let repo = init(&root, "repo");
    cli.ok(&repo, &["epic", "add", "tags", "--title", "Tags", "--brief", "-"], "Brief");
    for (number, tags) in [(1, ""), (2, "routine"), (3, "routine,judgment,frontend-design")] {
        let number = number.to_string();
        let mut args = vec!["ticket", "add", "tags", &number, "--title", "Work", "--body", "-"];
        if !tags.is_empty() {
            args.extend(["--tags", tags]);
        }
        cli.ok(&repo, &args, "Body");
    }
    assert!(!cli.ok(&repo, &["ticket", "tags/1"], "").contains("Tags:"));
    assert!(cli.ok(&repo, &["ticket", "tags/2"], "").contains("Tags: routine\n"));
    assert!(cli.ok(&repo, &["ticket", "tags/3"], "").contains("Tags: routine, judgment, frontend-design\n"));
    for tags in ["unknown", "routine,routine"] {
        let error = cli.refused(&repo, &["ticket", "add", "tags", "4", "--title", "Bad", "--tags", tags, "--body", "-"], "Body");
        assert!(error.contains(if tags == "unknown" { "invalid value" } else { "must be unique" }), "{error}");
    }
    cli.ok(&repo, &["ticket", "set-tags", "tags/2", "judgment,frontend-design"], "");
    cli.ok(&repo, &["ticket", "set-tags", "tags/3"], "");
    let before = cli.ok(&repo, &["ticket", "tags/2"], "");
    assert!(before.contains("Tags: judgment, frontend-design\n"));
    assert!(!cli.ok(&repo, &["ticket", "tags/3"], "").contains("Tags:"));
    server.restart().await;
    assert_eq!(cli.ok(&repo, &["ticket", "tags/2"], ""), before);
    assert!(!cli.ok(&repo, &["ticket", "tags/3"], "").contains("Tags:"));
}

#[tokio::test]
async fn the_cli_publishes_and_reads_an_epic_through_the_server() {
    let mut server = Server::start("loop-cli").await;
    let root = server.root.path().to_owned();
    let cli = Cli { data_dir: root.join("data") };
    let repo = init(&root, "repo");
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "Initial commit"]);
    let worktree = root.join("worktree");
    git(&repo, &["worktree", "add", "-q", worktree.to_str().unwrap()]);
    fs::create_dir(worktree.join("sub")).unwrap();

    let brief = "# The loop\n\nEpics and Tickets inside Stampede.\n";
    let added = cli.ok(&worktree.join("sub"), &["epic", "add", "loop", "--title", "The loop", "--brief", "-"], brief);
    assert_eq!(added, format!("Added Epic loop to the Project at {}.\n", repo.join(".git").display()));
    assert_eq!(cli.ok(&repo, &["brief", "loop"], ""), brief);

    assert_eq!(cli.ok(&repo, &["ticket", "add", "loop", "1", "--title", "The store", "--body", "-"], "Build the store.\n"), "Added loop/1.\n");
    cli.ok(&repo, &["ticket", "add", "loop", "2", "--title", "The tab", "--body", "-"], "Show it.\n");
    fs::write(root.join("three.md"), "Run it.\n").unwrap();
    let body = root.join("three.md");
    cli.ok(&repo, &["ticket", "add", "loop", "3", "--title", "Runs", "--blocked-by", "1,2", "--body", body.to_str().unwrap()], "");

    assert_eq!(
        cli.ok(&repo, &["ticket", "loop/3"], ""),
        "# loop/3: Runs\n\nStatus: not started\nBlocked by: loop/1 (not started), loop/2 (not started)\n\nRun it.\n"
    );
    assert_eq!(cli.ok(&repo, &["note", "loop/3", "Decided on one socket.\nLine two."], ""), "Added a Note to loop/3.\n");
    cli.ok(&repo, &["ticket", "set-body", "loop/1", "-"], "Build the store first.\n");
    cli.ok(&repo, &["epic", "set-brief", "loop", "-"], "# The loop\n\nRevised.\n");
    assert_eq!(cli.ok(&repo, &["epic", "set-setup", "loop", ""], ""), "Recorded the Setup Command of loop.\n");
    cli.ok(&repo, &["epic", "set-setup", "loop", "pnpm install"], "");
    assert_eq!(cli.ok(&repo, &["frontier", "loop"], ""), "loop/1 The store\nloop/2 The tab\n");

    let other = init(&root, "other");
    let refusals: [(&Path, &[&str], &str, &str); 9] = [
        (&other, &["epic", "add", "loop", "--title", "Again", "--brief", "-"], "Brief", "Epic loop already exists in"),
        (&root, &["epic", "add", "plain", "--title", "Plain", "--brief", "-"], "Brief", "is not inside a git repository"),
        (&repo, &["epic", "add", "Not A Slug", "--title", "T", "--brief", "-"], "Brief", "is not a valid slug"),
        (&repo, &["epic", "add", "empty", "--title", "T", "--brief", "-"], "", "the Brief is empty"),
        (&repo, &["ticket", "add", "loop", "3", "--title", "Again", "--body", "-"], "Body", "loop/3 already exists"),
        (&repo, &["ticket", "add", "loop", "4", "--title", "T", "--blocked-by", "7", "--body", "-"], "Body", "loop has no Ticket 7 to block loop/4"),
        (&repo, &["ticket", "loop/9"], "", "loop has no Ticket 9"),
        (&repo, &["brief", "nope"], "", "there is no Epic nope"),
        (&repo, &["note", "nope/1", "text"], "", "there is no Epic nope"),
    ];
    for (cwd, args, stdin, expected) in refusals {
        let error = cli.refused(cwd, args, stdin);
        assert!(error.starts_with("error: ") && error.contains(expected), "{args:?}: {error}");
    }

    let log = fs::read_to_string(root.join("data/epics/loop.jsonl")).unwrap();
    let kinds: Vec<String> = log
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["kind"].as_str().unwrap().to_owned())
        .collect();
    let expected = ["epic.added", "ticket.added", "ticket.added", "ticket.added", "note.added", "ticket.body.set", "brief.set", "setup.set", "setup.set"];
    assert_eq!(kinds, expected);
    assert!(!root.join("data/epics/plain.jsonl").exists());

    server.restart().await;
    assert_eq!(
        cli.ok(&repo, &["ticket", "loop/3"], ""),
        "# loop/3: Runs\n\nStatus: not started\nBlocked by: loop/1 (not started), loop/2 (not started)\n\nRun it.\n\n## Notes\n\n- agent: Decided on one socket.\n  Line two.\n"
    );
    assert_eq!(cli.ok(&repo, &["ticket", "loop/1"], ""), "# loop/1: The store\n\nStatus: not started\nBlocked by: nothing\n\nBuild the store first.\n");
    assert_eq!(cli.ok(&repo, &["brief", "loop"], ""), "# The loop\n\nRevised.\n");
    assert_eq!(cli.ok(&repo, &["frontier", "loop"], ""), "loop/1 The store\nloop/2 The tab\n");
    cli.ok(&other, &["epic", "add", "other", "--title", "Other", "--brief", "-"], "Brief");
    assert_eq!(cli.ok(&other, &["frontier", "other"], ""), "Nothing in other is on the Frontier.\n");
}
