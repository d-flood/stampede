Break an Epic's Brief into independently-grabbable **tracer bullet** Tickets, each naming the Tickets that **block** it, and publish them in Oikonomos. Then propose the Epic's Setup Command, verify it, and record it once the user confirms it.

## 1. Gather context

Work from the conversation. The Epic is the one created in this conversation, unless the text after these instructions names another. Read its Brief with `oikonomos brief <epic>`. When the user names a Ticket, read its full body with `oikonomos ticket <epic>/<n>`.

## 2. Explore the codebase

Explore enough to fill each Ticket's "Where to start" pointers and acceptance-criteria commands: which files and functions each slice touches, and which build, test and run commands actually work in this repo. The implementer should spend its effort on the change, not on rediscovering the codebase. Use the vocabulary of the project's glossary (`GLOSSARY.md`, or `CONTEXT.md` in older repos), and respect its ADRs (`docs/adr/`) in the area you're touching.

Look for prefactoring that makes the change easy: "Make the change easy, then make the easy change."

## 3. Draft the slices

- Each slice is a narrow but COMPLETE path through every layer (schema, API, UI, checks), demoable or verifiable on its own.
- Each uses most of a fresh context window, not merely fits one: merge neighbouring slices that touch the same modules into one Ticket, and stage their acceptance criteria.
- Prefactoring goes first.
- A single mechanical change with codebase-wide blast radius is a wide refactor instead (below).

Give each Ticket its Blockers; a Ticket with none can be taken immediately.

Give each Ticket zero or more of these tags, independently: `routine` (a clear contract and predictable implementation), `judgment` (requires autonomous decisions during implementation), and `frontend-design` (includes visual or interaction design). Tags may overlap; leave them empty when none apply. For `judgment`, name the open decisions and their constraints in the Contract. Tags describe the work; David chooses its Rig.

When the Brief has user stories, **every story is covered by exactly one Ticket**: the one that makes it true end to end. Check coverage both ways: every story in the Brief appears on some Ticket, and every story cited on a Ticket exists in the Brief. Report any story you could not place and why, rather than dropping it or splitting it so that no single Ticket delivers it.

## 4. Publish

Publish each slice with the Ticket template below. Number the Tickets from 1, in dependency order, since a Blocker must already exist when the Ticket it blocks is added:

```sh
oikonomos ticket add <epic> <n> --title "<one-line title>" --blocked-by 1,3 --tags judgment,frontend-design --body - <<'EOF'
<the Ticket's body>
EOF
```

Leave out `--blocked-by` when nothing blocks the Ticket, and `--tags` when none apply; the tags above are an example, not defaults. Quote the heredoc's delimiter, as above, so that the shell leaves the body's backticks and dollar signs alone. Once every Ticket is added, `oikonomos frontier <epic>` should list exactly the Tickets that nothing blocks. To revise a published Ticket, pipe its whole new body to `oikonomos ticket set-body <epic>/<n> -`, which replaces the old one. Replace its tags with `oikonomos ticket set-tags <epic>/<n> routine,frontend-design`, or omit the tags to clear them.

Write each Ticket for an implementer less capable than you. The Chat that works it is given the Ticket's body and can read the Brief, and nothing else of this conversation. Assume it will not re-derive context, will follow instructions literally, and will creep in scope unless fenced. Carry every decision you have already made into the Ticket, as constraints and contracts, not prescriptive diffs.

## 5. Propose the Setup Command

Every Ticket is worked in a fresh worktree of the Epic's branch, which holds only committed files. The Epic's **Setup Command** restores what such a checkout lacks: dependencies, build output, anything ignored that the Tickets' commands need. It runs through a shell in the root of every new worktree before any Chat starts there, and a Run refuses to start until the Epic has one.

1. **Decide what a fresh checkout needs** before the build, test and run commands in your Tickets' acceptance criteria work. Look for lockfiles and package manifests at the root and in subdirectories, and for the setup steps the project's README and scripts name. Name every step, chained with `&&`, such as `pnpm install && ./scripts/seed-fixtures`. When a fresh checkout already runs, the Setup Command is the empty string `""`.
2. **Verify it in a fresh clone.** A command that works in your own tree proves nothing: your tree has the ignored files a fresh worktree lacks. Clone the repository into a new temporary directory outside it and run the command there:

   ```sh
   git clone --no-hardlinks <repository> <clone> && cd <clone> && <command>
   ```

   It is verified when it exits 0 in that clone. Then delete the clone.
3. **Ask the user to confirm it,** showing the command and what the verification showed. A Setup Command that fails costs a Ticket: the Ticket is left untaken and needing attention, with the output's tail in a Note.
4. **Record it once the user confirms it,** and never before:

   ```sh
   oikonomos epic set-setup <epic> "<command>"
   ```

## Wide refactors

The exception to vertical slicing: one mechanical change (rename a column, retype a shared symbol) whose **blast radius** breaks too many call sites for any vertical slice to land green. Sequence it **expand–contract**: an expand Ticket adds the new form beside the old; migrate Tickets, each blocked by the expand, move call sites in batches sized by blast radius; a contract Ticket, blocked by every batch, deletes the old form. When even the batches can't stay green alone, let them share an integration branch that all block a final integrate-and-verify Ticket; green is promised only there.

## Ticket template

<ticket-template>
## What to build

The end-to-end behavior of this slice, not layer-by-layer implementation.

## Where to start

The key files, functions or modules this slice touches, and what to read first: hints to skip exploration, not instructions. For a blocked slice, prefer behavioral pointers ("the endpoint slice 1 adds"): the code will have changed by pickup time.

## Contract

The decisions that must hold: type shapes, API signatures, schema changes, state machines, and the Brief's testing posture with the tests or seams it names for this slice. Inline a snippet when it encodes a decision more precisely than prose, trimmed to the decision-rich parts. State boundaries other slices depend on as behavior ("the endpoint returns X"), not file contents.

## User Stories

The Brief's stories this Ticket satisfies, quoted by number and text. The slice is not done until each is true end to end. Omit only when the Brief has no stories.

## Out of scope

Adjacent refactors to leave alone, files not to touch, behavior that belongs to other slices. Name the tempting wrong turns, not the mechanical consequences of the change.

## Acceptance criteria

- [ ] Criterion 1
- [ ] Criterion 2

Each criterion mechanically checkable: a command that passes, an observable behavior. Then the exact commands that check them and what success looks like, runnable as written; the implementer and the reviewer both run them. Run each command before publishing: if a suite is already red or a script doesn't exist, fix the command or record the baseline here.
</ticket-template>

What follows these instructions, if anything, is from the user, such as the Epic to work from.
