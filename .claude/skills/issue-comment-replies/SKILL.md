---
name: issue-comment-replies
description: >
  Use whenever the user wants to catch up on or respond to GitHub issue or PR
  comments: "check the issues for new comments", "any comments to reply to?",
  "catch up on the tickets", or a board pass finding an unanswered comment.
  Scans open and recently closed issues and PRs for comments not yet
  answered, decides which merit a reply, AUTO-POSTS low-risk factual replies
  and DRAFTS substantive ones for the maintainer's OK. Every posted comment
  opens with the "🤖 Comment by Claude" attribution line. Trigger even if the
  user doesn't say "skill".
---

You triage new comments on this repo's issues and PRs and answer the ones
that merit it. Comments post under the maintainer's own account (@starquake)
through `gh`, so **every** reply opens with the attribution line, and a
public comment under someone else's name is hard to take back. Split replies
into *auto-post* (safe, factual) and *draft for approval* (substantive,
contestable). When unsure, it's a draft.

## The attribution line (never omit it)

Every comment you post starts with this exact line, then a blank line, then
the reply:

```
> 🤖 **Comment by Claude** (AI pair-programmer working with @starquake) — posted through @starquake's account.
```

Write the comment to a file and post with `gh issue comment <n> --body-file
<file>` (or `gh pr comment`): multi-line markdown with the `>` header breaks
`--body "…"` quoting.

## Step 1: find unanswered comments

Both the maintainer and Claude post as @starquake, so authorship can't tell
them apart; **the `> 🤖` prefix can**. A maintainer comment is a @starquake
comment that does NOT start with `> 🤖`.

1. List candidate threads (open, and touched in the last ~2 weeks):
   `gh issue list --state all --limit 40 --json number,title,updatedAt,state`
   and the same for `gh pr list`.
2. Pull each thread: `gh issue view <n> --json title,body,comments`.
3. **Unanswered** means a human comment (the maintainer's unmarked ones, or
   anyone else's) with no `> 🤖` comment after it. Answer blocks and go
   signals on gated tickets belong to `work-the-board` / `design-slice`:
   fold those in there, don't just reply.
4. Skip bot noise, and threads where a `> 🤖` reply already came last.

Report what you found before replying: "thread → unanswered comment".

## Step 2: which merit a reply

Reply to a direct question, an @-mention, or a technical claim where a
code-grounded answer helps. Don't reply to acknowledgements ("thanks", "👍"),
restatements of recorded decisions, or anything only a specific person can
answer. Silence is a fine outcome.

## Step 3: auto-post or draft

- **Auto-post**: factual and grounded. What the code does, where something
  lives, "tracked in #NN", a status, a clarifying question back.
- **Draft for approval**: opinion, pushback, anything that could read as a
  **decision** (decisions are the maintainer's), or anything you're not sure
  of. Show each draft with its thread, and post only the approved ones.

## Step 4: write it

Read the relevant code first, and never answer from memory. The answer goes in
the **first line**; reasoning goes below. Link the file, issue or PR you cite.

Example (auto-post, factual):

```
> 🤖 **Comment by Claude** (AI pair-programmer working with @starquake) — posted through @starquake's account.

Yes: only `BIT n,(HL)` reads MEMPTR, in `interp::execute`
(`crates/zx-runtime/src/interp.rs`), so a wrong MEMPTR rule can change flag
bits 3 and 5 after that one instruction and nothing else. That is why
starquake-recompiled#145 is a check rather than a fix.
```

## Guardrails

- The attribution line goes on every posted comment, with no exceptions.
- Never post a design decision, only grounded observations.
- Never write in the maintainer's own voice.
- Don't double-reply. Prefer a few high-value replies over covering every
  thread.
