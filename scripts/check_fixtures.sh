#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

OIKONOMOS_E2E_ROOT=${1:?}
OIKONOMOS_E2E_PORT=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
export OIKONOMOS_E2E_PORT OIKONOMOS_E2E_ROOT
mkdir "$OIKONOMOS_E2E_ROOT/data" "$OIKONOMOS_E2E_ROOT/projects"
mkdir "$OIKONOMOS_E2E_ROOT/projects/alpha" "$OIKONOMOS_E2E_ROOT/projects/beta"
alpha=(git -C "$OIKONOMOS_E2E_ROOT/projects/alpha" -c init.defaultBranch=main -c user.name=oikonomos -c user.email=e2e@oikonomos.invalid -c commit.gpgsign=false)
"${alpha[@]}" init -q
echo "# alpha" > "$OIKONOMOS_E2E_ROOT/projects/alpha/README.md"
echo 'export const greeting = "hello";' > "$OIKONOMOS_E2E_ROOT/projects/alpha/greeting.ts"
printf 'one\ntwo\nthree\n' | tee "$OIKONOMOS_E2E_ROOT/projects/alpha/"{desktop,pixel}.txt > /dev/null
mkdir "$OIKONOMOS_E2E_ROOT/projects/alpha/ignored"
echo 'ignored/' > "$OIKONOMOS_E2E_ROOT/projects/alpha/.gitignore"
for form in desktop pixel; do
    echo 'initial ignored contents' > "$OIKONOMOS_E2E_ROOT/projects/alpha/ignored/$form.txt"
    touch "$OIKONOMOS_E2E_ROOT/projects/alpha/empty-$form.ts"
done
"${alpha[@]}" add .
"${alpha[@]}" commit -qm "Initial commit"
echo "roots = [\"$OIKONOMOS_E2E_ROOT/projects\"]" > "$OIKONOMOS_E2E_ROOT/config.toml"
cat >> "$OIKONOMOS_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Fake Claude"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "simple-turn", CLAUDE_CONFIG_DIR = "$OIKONOMOS_E2E_ROOT/claude" }
shortcut = "c"

[[rig]]
name = "Fake Claude (steer)"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "steer-queue-interrupt" }
EOF
cat >> "$OIKONOMOS_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Fake Codex"
agent = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "simple-turn", OIKONOMOS_FAKE_THREADS = "threads", CODEX_HOME = "$OIKONOMOS_E2E_ROOT/codex" }

[[rig]]
name = "Fake Claude Tools"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "tools", CLAUDE_CONFIG_DIR = "$OIKONOMOS_E2E_ROOT/claude" }

[[rig]]
name = "Fake Claude Edits"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "edits", CLAUDE_CONFIG_DIR = "$OIKONOMOS_E2E_ROOT/claude" }

[[rig]]
name = "Fake Codex Edits"
agent = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "edits", CODEX_HOME = "$OIKONOMOS_E2E_ROOT/codex" }

[[rig]]
name = "Claude Approval"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "approval", CLAUDE_CONFIG_DIR = "$OIKONOMOS_E2E_ROOT/claude" }

[[rig]]
name = "Codex Approval"
agent = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "approval", CODEX_HOME = "$OIKONOMOS_E2E_ROOT/codex" }
EOF

for form in desktop pixel; do
mkdir "$OIKONOMOS_E2E_ROOT/subagent-$form" "$OIKONOMOS_E2E_ROOT/gated-$form"
cat >> "$OIKONOMOS_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Fake Claude Gated $form"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "$PWD/crates/oikonomos/tests/fixtures/chats/claude-code/verdict.synthetic.jsonl", OIKONOMOS_FAKE_GATES = "$OIKONOMOS_E2E_ROOT/gated-$form", CLAUDE_CONFIG_DIR = "$OIKONOMOS_E2E_ROOT/claude" }

[[rig]]
name = "Fake Claude Subagent $form"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "subagents", OIKONOMOS_FAKE_GATES = "$OIKONOMOS_E2E_ROOT/subagent-$form", CLAUDE_CONFIG_DIR = "$OIKONOMOS_E2E_ROOT/claude" }

[[rig]]
name = "Fake Codex Subagent $form"
agent = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "subagents", CODEX_HOME = "$OIKONOMOS_E2E_ROOT/codex" }
EOF
for agent in claude codex; do
mkdir "$OIKONOMOS_E2E_ROOT/activity-$agent-$form"
kind="$agent"
if [[ "$agent" == claude ]]; then kind=claude-code; fi
cat >> "$OIKONOMOS_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Activity $agent $form"
agent = "$kind"
env = { OIKONOMOS_FAKE_FIXTURE = "$OIKONOMOS_E2E_ROOT/activity-$agent-$form/tools.jsonl", OIKONOMOS_FAKE_GATES = "$OIKONOMOS_E2E_ROOT/activity-$agent-$form", CLAUDE_CONFIG_DIR = "$OIKONOMOS_E2E_ROOT/claude", CODEX_HOME = "$OIKONOMOS_E2E_ROOT/codex" }
EOF
done
done

cat >> "$OIKONOMOS_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Claude Questions"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "$PWD/crates/oikonomos/tests/fixtures/chats/claude-code/questions.synthetic.jsonl", CLAUDE_CONFIG_DIR = "$OIKONOMOS_E2E_ROOT/claude" }

[[rig]]
name = "Fake Claude Verdict"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "$PWD/crates/oikonomos/tests/fixtures/chats/claude-code/verdict.synthetic.jsonl", CLAUDE_CONFIG_DIR = "$OIKONOMOS_E2E_ROOT/claude" }

[[rig]]
name = "Codex Questions"
agent = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "questions", CODEX_HOME = "$OIKONOMOS_E2E_ROOT/codex" }
EOF

cat >> "$OIKONOMOS_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Claude Rewind"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "$PWD/crates/oikonomos/tests/fixtures/chats/claude-code/rewind.synthetic.jsonl", CLAUDE_CONFIG_DIR = "$OIKONOMOS_E2E_ROOT/claude" }

[[rig]]
name = "Codex Rewind"
agent = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "$PWD/crates/oikonomos/tests/fixtures/chats/codex/rewind.synthetic.jsonl", CODEX_HOME = "$OIKONOMOS_E2E_ROOT/codex" }
EOF

for agent in claude-code codex; do
cat >> "$OIKONOMOS_E2E_ROOT/config.toml" <<EOF

[[rig]]
name = "Rich $agent Questions"
agent = "$agent"
env = { OIKONOMOS_FAKE_FIXTURE = "$PWD/crates/oikonomos/tests/fixtures/chats/$agent/rich-questions.synthetic.jsonl", CLAUDE_CONFIG_DIR = "$OIKONOMOS_E2E_ROOT/claude", CODEX_HOME = "$OIKONOMOS_E2E_ROOT/codex" }
EOF
done

printf "%s\n" "$OIKONOMOS_E2E_PORT"
