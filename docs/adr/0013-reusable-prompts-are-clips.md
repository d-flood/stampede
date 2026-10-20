# Reusable prompts are Clips inserted into the Composer

Prompts David repeats are Clips: named text kept in `config.toml` and inserted at the caret of any Composer by typing `;` and the Clip's name, where it can be edited before sending.

- Agents treat skills inconsistently: some honour front matter that hides a skill, others only refuse to invoke it. A Clip is plain text in the Composer, so it behaves the same for every Agent, a TUI, or a shell.
- Templates stay Stampede's own: shipped with the code, expanded by the server, and recognized in the transcript. Clips are David's, so they live in the config directory, where Templates were refused a place.
- `;` rather than `/`: a Clip can go anywhere in the text, not only at its start, and its names never clash with built-in commands, Templates, or an Agent's commands. `;` is unshifted on desktop keyboards, on the first symbols page of phone keyboards, and almost never follows a space in prose, code, or a shell.
- Accepted cost: Clips have no placeholders; the text is edited in the Composer instead.
- Rejected: Clips in the slash menu, which ties them to the start of the text.
- Rejected: per-Project Clips committed to a repository.
