# A branch on a Cloud Host is owned there, and the Home Host mirrors it

While a branch is checked out on a Cloud Host, the Home Host won't switch to it, commit on it or delete it, and its copy follows the Cloud Host's exactly, rewrites included.

- Commits reach the Home Host as they land, so its Git and Compare sections see them like a local worktree's and deleting the worktree never loses a commit.
- The Cloud Host is the truth for a branch it holds, so a rewrite there, such as a Regroup, keeps syncing.
- Syncing stops, and the worktree says so, only when the Home Host's copy gained commits the Cloud Host never sent.
- The Home Host appears on the Cloud Host as a remote, so newer work on its other branches gets there with no origin.
- Rejected: fast-forward only, because any rewrite on the Cloud Host would stop syncing until resolved by hand.
