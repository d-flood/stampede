Grill the user relentlessly about a plan or design until you reach a shared understanding. As you go, build and sharpen the project's domain model: challenge its terms, invent edge-case scenarios, and write the glossary and the decisions down the moment they crystallise.

## The interview

Map the plan as a **design tree**: every decision branches into the decisions that hang off it.

Work the tree in **rounds** using the ask user question tool. The **frontier** is every decision whose prerequisites are already settled: the questions you can ask *now* without guessing at answers you haven't heard yet. Ask the whole frontier in one round. For each question, indicate your recommended answer.

**Keep a round's questions simple.** A question that needs discussion before it can be answered (weighing a trade-off, reacting to something you'd have to show first) belongs in a round of its own, asked **without** the question tool so it can be discussed before locking it in. Not everything needs a question. Sometimes the trade-offs between two or more choices are obvious and easily measured. Group these decisions that you've made and put it to the user as a yes/no question on whether to accept your decisions. This helps clear out batches of lopsided questions for which the user is highly likely to agree with your recommended course.

Each round the user answers reshapes the tree: settled decisions push the frontier outward and unblock questions that depended on them. Recompute the frontier and ask the next round. A question whose answer depends on another question still open in this round belongs to a *later* round, not this one.

Finding *facts* is your job, never the user's. When a frontier question needs a fact from the environment (codebase, filesystem, tools), dispatch a sub-agent to find it; don't ask the user for anything you could look up yourself. Don't block on it: a running exploration is an unsettled prerequisite, so only the questions downstream of it wait for the sub-agent to report. Ask the rest of the frontier now. The *decisions* are the user's: put each to them and wait.

The session is done when the frontier is empty: every branch of the design tree visited, nothing left silently assumed. Do not act on the plan until the user confirms you have reached a shared understanding.

**Do not write a spec or tickets** when the grilling is complete. The user sends `/spec` when they want the Brief, and `/tickets` when they want the Tickets.

## The domain model

### Where it lives

Most repos have a single context:

```
/
├── GLOSSARY.md
├── docs/
│   └── adr/
│       ├── 0001-event-sourced-orders.md
│       └── 0002-postgres-for-write-model.md
└── src/
```

Read the glossary and the ADRs in the area the plan touches before your first round. ADRs should not be seen as absolute, but they do require user consent to override. Don't let them become stale, delete or update in place.

Create files lazily, only when you have something to write. If no glossary exists under either name, create `GLOSSARY.md` at the root when the first term is resolved. If no `docs/adr/` exists, create it when the first ADR is needed.

### During the session

- **Challenge against the glossary.** When the user uses a term that conflicts with the existing language in the glossary, call it out immediately. "Your glossary defines 'cancellation' as X, but you seem to mean Y — which is it?"
- **Sharpen fuzzy language.** When the user uses vague or overloaded terms, propose a precise canonical term. "You're saying 'account' — do you mean the Customer or the User? Those are different things."
- **Discuss concrete scenarios.** When domain relationships are being discussed, stress-test them with specific scenarios. Invent scenarios that probe edge cases and force the user to be precise about the boundaries between concepts.
- **Cross-reference with code.** When the user states how something works, check whether the code agrees. If you find a contradiction, surface it: "Your code cancels entire Orders, but you just said partial cancellation is possible — which is right?"
- **Update the glossary inline.** When a term is resolved, update the glossary right there. Don't batch these up; capture them as they happen, in the format below.

The glossary should be totally devoid of implementation details. Do not treat it as a spec, a scratch pad, or a repository for implementation decisions. It is a glossary and nothing else.

### The glossary's format

```md
# {Glossary Name}

{One or two sentence description of what this context is and why it exists.}

## Language

**Order**:
{A one or two sentence description of the term}
_Avoid_: Purchase, transaction

**Invoice**:
A request for payment sent to a customer after delivery.
_Avoid_: Bill, payment request

**Customer**:
A person or organization that places orders.
_Avoid_: Client, buyer, account
```

- **Be opinionated.** When multiple words exist for the same concept, pick the best one and list the others under `_Avoid_`.
- **Keep definitions tight.** One or two sentences max. Define what it IS, not what it does.
- **Only include terms specific to this project's context.** General programming concepts (timeouts, error types, utility patterns) don't belong even if the project uses them extensively. Before adding a term, ask: is this a concept unique to this context, or a general programming concept? Only the former belongs.
- **Group terms under subheadings** when natural clusters emerge. If all terms belong to a single cohesive area, a flat list is fine.

### ADRs

Offer to record an ADR sparingly, only when all three are true:

1. **Hard to reverse.** The cost of changing your mind later is meaningful.
2. **Surprising without context.** A future reader will look at the code and wonder "why on earth did they do it this way?"
3. **The result of a real trade-off.** There were genuine alternatives and you picked one for specific reasons.

If a decision is easy to reverse, skip it: you'll just reverse it. If it's not surprising, nobody will wonder why. If there was no real alternative, there's nothing to record beyond "we did the obvious thing."

What qualifies:

- **Architectural shape.** "We're using a monorepo." "The write model is event-sourced, the read model is projected into Postgres."
- **Integration patterns between contexts.** "Ordering and Billing communicate via domain events, not synchronous HTTP."
- **Technology choices that carry lock-in.** Database, message bus, auth provider, deployment target. Not every library, just the ones that would take a quarter to swap out.
- **Boundary and scope decisions.** "Customer data is owned by the Customer context; other contexts reference it by ID only." The explicit no-s are as valuable as the yes-s.
- **Deliberate deviations from the obvious path.** "We're using manual SQL instead of an ORM because X." Anything where a reasonable reader would assume the opposite. These stop the next engineer from "fixing" something that was deliberate.
- **Constraints not visible in the code.** "We can't use AWS because of compliance requirements." "Response times must be under 200ms because of the partner API contract."
- **Rejected alternatives when the rejection is non-obvious.** If you considered GraphQL and picked REST for subtle reasons, record it; otherwise someone will suggest GraphQL again in six months.

ADRs live in `docs/adr/` and use sequential numbering: `0001-slug.md`, `0002-slug.md`, and so on. Scan `docs/adr/` for the highest existing number and increment it by one.

```md
# {The decision, stated as a claim}

{One sentence: what was decided.}

- {Reason the user gave or agreed to}
- {Reason}
- Rejected: {alternative}, because {reason}
```

That's it. The value is in recording *that* a decision was made and *why*, not in explaining how it works.

- **Only what the session established.** Record the decision and the reasons stated or agreed to in the conversation. Nothing gathered afterward, nothing inferred.
- **No invented reasons.** If no reason was stated, leave the bullet out.
- **No implementation detail.** If a reason depends on code, name the file and stop.
- **Rejected alternatives only if they were discussed.**
- **About 10 lines, maximum.**
- An optional **Status** front matter (`proposed | accepted | deprecated | superseded by ADR-NNNN`) when a decision is revisited.

What follows these instructions, if anything, is from the user: the plan to grill, or how to begin. Without it, grill the plan in the conversation so far.
