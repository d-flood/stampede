# A Cloud Setup is committed in the repository, unlike an Epic's Setup Command

The recipe for a fresh Cloud Host lives in one small file under `.stampede/` in the Project's repository, naming the ignored files to carry from the Home Host and one command to run.

- It is versioned with the code and shared with collaborators.
- The recipe that runs is always the one in the worktree's own commit, so a branch without one can't go to a Cloud Host.
- It names the ignored files to carry because only the Home Host's checkout shows which exist; a Chat there drafts it.
- An Epic's Setup Command stays in Stampede, decided when its Tickets are written.
- Rejected: keeping the Cloud Setup in Stampede per Project.
- Rejected: `devcontainer.json`, because it needs Docker on every Cloud Host, which Fly doesn't document for Sprites.
