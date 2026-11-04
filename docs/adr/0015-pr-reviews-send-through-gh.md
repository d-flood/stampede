# PR reviews reach GitHub only through gh, and only to send

Stampede talks to GitHub through the installed, signed-in `gh` CLI, and a PR review only sends reviews: it never shows, replies to, or resolves existing threads.

- Stampede stores no GitHub tokens; it reuses `gh`'s sign-in.
- Sending Draft comments as one review covers the review flow.
- Rejected: a token or OAuth app owned by Stampede, because it means storing and managing credentials.
- Rejected: two-way threads, because placing comments on older commits, replying, and resolving cost far more than sending.
