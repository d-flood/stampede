Inspect this Project's checkout on the Home Host and draft its Cloud Setup for David to review and commit.

## Inspect the Project

- List ignored paths with git (for example, `git ls-files --others --ignored --exclude-standard`). Identify the files the Project needs at run time, such as env files and local config. Never carry dependencies or build output.
- Identify the tools and versions the Project uses, its package managers and the services it expects, such as a database.
- Read the Project's README and existing setup scripts. Reuse its own setup commands where they fit.

## Know the Cloud Host

The following was verified on a fresh Fly.io Sprite on 2026-11-24:

- Ubuntu 26.04, x86_64, 8 CPUs, 15 GB memory, user `sprite` with passwordless sudo, no systemd.
- Installed: git, gh, Node 24, npm, Python 3.13, uv, Rust and cargo, Go, and the Claude Code and Codex CLIs.
- Missing: pnpm and Docker.
- Docker installs with apt, but the recipe must start its daemon. Containers run and published ports work; `docker exec` fails, so do not depend on it.
- Stampede has already put the repository, the carried files, the Agents and the GitHub login in place before the Cloud Setup runs.

## Write the Cloud Setup

At the repository root, write `.stampede/cloud-setup.toml` with this shape, adapted to the Project:

```toml
carry = [".env", "config/local.toml"]  # ignored paths, relative to the repository root
run = "./scripts/cloud-setup"          # one command, run with sh in the worktree on the Cloud Host
```

Keep `run` non-interactive, safe to run twice, and exit non-zero on failure. Include no secrets in the recipe; secrets travel through `carry`, not as literal values.

Write only the Cloud Setup file, plus one script in the repository if the recipe is too long for a single inline command. Do not modify other files.

Do not commit or create a Cloud Host. Tell David to review and commit the Cloud Setup, then try it by making a worktree on a Cloud Host from that branch.
