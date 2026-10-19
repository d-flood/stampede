Synthesize this conversation and your understanding of the codebase into a spec, and create it in Stampede as the Brief of a new Epic. Do NOT interview the user.

1. Explore the repo if you haven't. Use the vocabulary of the project's glossary (`GLOSSARY.md`, or `CONTEXT.md` in older repos), and respect its ADRs (`docs/adr/`) in the area you're touching.
2. Write the Brief from the template below.
3. Create the Epic from inside the repository it will change, since an Epic belongs to the Project of the directory the command runs in:

   ```sh
   stampede epic add <slug> --title "<one-line title>" --brief - <<'EOF'
   <the Brief>
   EOF
   ```

   The slug names the Epic in every reference, such as `<slug>/4` for its fourth Ticket, so keep it short: lowercase letters, digits and hyphens, such as `chat-ui`. No other Epic in any Project may already use it; if it is refused, choose another. Quote the heredoc's delimiter, as above, so that the shell leaves the Brief's backticks and dollar signs alone.

To revise the Brief afterwards, pipe the whole new Brief to `stampede epic set-brief <slug> -`, which replaces the old one.

<brief-template>

## Problem Statement

The problem, from the user's perspective.

## Solution

The solution, from the user's perspective.

## User Stories

A LONG, numbered list: `1. As an <actor>, I want a <feature>, so that <benefit>`.

Never renumber: Tickets cite stories by number. To drop a story, mark it withdrawn in place.

## Implementation Decisions

The decisions made: modules and their interfaces, architecture, schema, API contracts, specific interactions, the developer's technical clarifications.

No file paths: the spec outlives them. Snippets that encode a decision more precisely than prose (schema, API contract, type shape, state machine) are welcome, trimmed to the decision-rich parts. No prescriptive diffs.

## Testing Decisions

The **posture**: `none`, `load-bearing` (default) or `tdd`, from the conversation or the text after these instructions. Then what it commits to:

- `none`: per story, the command that exercises it.
- `load-bearing`: each test, naming the invariant it guards and the seam it lives at.
- `tdd`: the seams the loop tests at.

Prefer existing seams and the highest one that reaches the behaviour; one is the ideal count. Point at prior art in the codebase.

## Out of Scope

## Further Notes

</brief-template>

Once the Epic exists, tell the user its slug, then **STOP**. Write no Tickets until the user sends `/tickets`.

What follows these instructions, if anything, is from the user, such as the testing posture.
