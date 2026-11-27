#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

STAMPEDE_E2E_ROOT=${1:?}
STAMPEDE_E2E_PORT=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
export STAMPEDE_E2E_PORT STAMPEDE_E2E_ROOT
mkdir "$STAMPEDE_E2E_ROOT/data" "$STAMPEDE_E2E_ROOT/projects"
mkdir "$STAMPEDE_E2E_ROOT/projects/alpha" "$STAMPEDE_E2E_ROOT/projects/beta"
alpha=(git -C "$STAMPEDE_E2E_ROOT/projects/alpha" -c init.defaultBranch=main -c user.name=stampede -c user.email=e2e@stampede.invalid -c commit.gpgsign=false)
"${alpha[@]}" init -q
echo "# alpha" > "$STAMPEDE_E2E_ROOT/projects/alpha/README.md"
echo 'export const greeting = "hello";' > "$STAMPEDE_E2E_ROOT/projects/alpha/greeting.ts"
printf 'one\ntwo\nthree\n' | tee "$STAMPEDE_E2E_ROOT/projects/alpha/"{desktop,pixel}.txt > /dev/null
mkdir "$STAMPEDE_E2E_ROOT/projects/alpha/ignored"
echo 'ignored/' > "$STAMPEDE_E2E_ROOT/projects/alpha/.gitignore"
mkdir "$STAMPEDE_E2E_ROOT/projects/alpha/.stampede"
echo 'run = "true"' > "$STAMPEDE_E2E_ROOT/projects/alpha/.stampede/cloud-setup.toml"
for form in desktop pixel; do
    echo 'initial ignored contents' > "$STAMPEDE_E2E_ROOT/projects/alpha/ignored/$form.txt"
    touch "$STAMPEDE_E2E_ROOT/projects/alpha/empty-$form.ts"
done
"${alpha[@]}" add .
"${alpha[@]}" commit -qm "Initial commit"
echo '{"organization":"test-org"}' > "$STAMPEDE_E2E_ROOT/sprite.json"
echo "roots = [\"$STAMPEDE_E2E_ROOT/projects\"]" > "$STAMPEDE_E2E_ROOT/config.toml"
cat >> "$STAMPEDE_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Fake Claude"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "simple-turn", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }
shortcut = "c"

[[rig]]
name = "Fake Claude (steer)"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "steer-queue-interrupt" }
EOF
python3 - "$STAMPEDE_E2E_ROOT" <<'PYTHON'
import json
import sys
import uuid
from pathlib import Path

root = Path(sys.argv[1])
threads = json.loads(Path("crates/stampede/tests/fixtures/chats/codex/threads.recorded.json").read_text())
listed = []
for form in ("desktop", "pixel"):
    for thread in threads:
        if thread["cwd"] != "/tmp/stampede-chat":
            continue
        thread = dict(thread, cwd=str(root / "projects" / f"sessions-{form}"))
        if form == "pixel":
            thread["id"] = thread["sessionId"] = str(uuid.uuid5(uuid.NAMESPACE_URL, thread["id"]))
        listed.append(thread)
(root / "codex-threads.json").write_text(json.dumps(listed))
PYTHON
cat >> "$STAMPEDE_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Fake Codex"
agent = "codex"
env = { STAMPEDE_FAKE_FIXTURE = "simple-turn", STAMPEDE_FAKE_THREADS = "$STAMPEDE_E2E_ROOT/codex-threads.json", CODEX_HOME = "$STAMPEDE_E2E_ROOT/codex" }

[[rig]]
name = "Fake Claude Tools"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "tools", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }

[[rig]]
name = "Fake Claude Workflow"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "workflow", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }

[[rig]]
name = "Fake Claude Edits"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "edits", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }

[[rig]]
name = "Fake Codex Edits"
agent = "codex"
env = { STAMPEDE_FAKE_FIXTURE = "edits", CODEX_HOME = "$STAMPEDE_E2E_ROOT/codex" }

[[rig]]
name = "Claude Approval"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "approval", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }

[[rig]]
name = "Codex Approval"
agent = "codex"
env = { STAMPEDE_FAKE_FIXTURE = "approval", CODEX_HOME = "$STAMPEDE_E2E_ROOT/codex" }
EOF

for form in desktop pixel; do
mkdir "$STAMPEDE_E2E_ROOT/subagent-$form" "$STAMPEDE_E2E_ROOT/gated-$form" "$STAMPEDE_E2E_ROOT/forked-$form"
cat >> "$STAMPEDE_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Fake Claude Gated $form"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "$PWD/crates/stampede/tests/fixtures/chats/claude-code/verdict.synthetic.jsonl", STAMPEDE_FAKE_GATES = "$STAMPEDE_E2E_ROOT/gated-$form", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }

[[rig]]
name = "Fake Claude Subagent $form"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "subagents", STAMPEDE_FAKE_GATES = "$STAMPEDE_E2E_ROOT/subagent-$form", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }

[[rig]]
name = "Fake Claude Forked $form"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "$PWD/crates/stampede/tests/fixtures/chats/claude-code/forked-skill.synthetic.jsonl", STAMPEDE_FAKE_GATES = "$STAMPEDE_E2E_ROOT/forked-$form", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }

[[rig]]
name = "Fake Codex Subagent $form"
agent = "codex"
env = { STAMPEDE_FAKE_FIXTURE = "subagents", CODEX_HOME = "$STAMPEDE_E2E_ROOT/codex" }
EOF
for agent in claude codex; do
mkdir "$STAMPEDE_E2E_ROOT/activity-$agent-$form"
kind="$agent"
if [[ "$agent" == claude ]]; then kind=claude-code; fi
cat >> "$STAMPEDE_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Activity $agent $form"
agent = "$kind"
env = { STAMPEDE_FAKE_FIXTURE = "$STAMPEDE_E2E_ROOT/activity-$agent-$form/tools.jsonl", STAMPEDE_FAKE_GATES = "$STAMPEDE_E2E_ROOT/activity-$agent-$form", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude", CODEX_HOME = "$STAMPEDE_E2E_ROOT/codex" }
EOF
done
done

cat >> "$STAMPEDE_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Claude Questions"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "$PWD/crates/stampede/tests/fixtures/chats/claude-code/questions.synthetic.jsonl", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }

[[rig]]
name = "Fake Claude Verdict"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "$PWD/crates/stampede/tests/fixtures/chats/claude-code/verdict.synthetic.jsonl", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }

[[rig]]
name = "Fake Reviewer"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "$PWD/crates/stampede/tests/fixtures/chats/claude-code/reviewer.synthetic.jsonl", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }

[[rig]]
name = "Fake Remediator"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "$PWD/crates/stampede/tests/fixtures/chats/claude-code/remediator.synthetic.jsonl", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }

[[rig]]
name = "Codex Questions"
agent = "codex"
env = { STAMPEDE_FAKE_FIXTURE = "questions", CODEX_HOME = "$STAMPEDE_E2E_ROOT/codex" }
EOF

cat >> "$STAMPEDE_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Claude Rewind"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "$PWD/crates/stampede/tests/fixtures/chats/claude-code/rewind.synthetic.jsonl", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude" }

[[rig]]
name = "Codex Rewind"
agent = "codex"
env = { STAMPEDE_FAKE_FIXTURE = "$PWD/crates/stampede/tests/fixtures/chats/codex/rewind.synthetic.jsonl", CODEX_HOME = "$STAMPEDE_E2E_ROOT/codex" }
EOF

for agent in claude-code codex; do
cat >> "$STAMPEDE_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Rich $agent Questions"
agent = "$agent"
env = { STAMPEDE_FAKE_FIXTURE = "$PWD/crates/stampede/tests/fixtures/chats/$agent/rich-questions.synthetic.jsonl", CLAUDE_CONFIG_DIR = "$STAMPEDE_E2E_ROOT/claude", CODEX_HOME = "$STAMPEDE_E2E_ROOT/codex" }
EOF
done

printf "%s\n" "$STAMPEDE_E2E_PORT"
