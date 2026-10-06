---
name: build-slice
description: >
  Use whenever an approved ticket gets built: "build #NN", "go ahead and
  implement it", "execute the plan", "resume #NN", a ticket moved to `Build`
  on the board, or a `go` / `approved` comment on a `Your sign-off` ticket.
  Also for bugs and tweaks routed `needs: build`. Executes the plan task by
  task on ONE PR: branch, failing tests first, the check gate green per commit,
  CI watched after every push, then the ticket moves to `Your review`. Merging
  stays gated on the `ready to merge` label. Trigger even if the user doesn't
  say "skill".
---

You execute an approved plan from a ticket, task by task, on one PR.
Precondition: the issue's *Plan* exists and the maintainer has approved it (or
it's a bug/tweak routed `needs: build`). If not, stop and route to
`design-slice`: never invent a plan mid-build.

## Setup

- Set the card to `Build` (`.claude/scripts/board.sh state <n> "Build"`).
- Branch from an up-to-date `main`, named for the ticket. In a worktree, run
  every `git` command inside it; never `git checkout` in the shared checkout
  while another agent may be working there.
- Re-verify the plan against the current tree before the first commit: the
  named symbols still exist, and nothing merged since moved the ground. Surface
  drift rather than improvising.
- **Resuming?** The issue's ticked checkboxes plus the branch's commits are
  the progress record. Confirm they agree, and pick up at the first unticked
  task.

## The task loop

1. **Failing test first** where the plan says so, and confirm it fails for the
   right reason. A test that can never run (skipped, unreachable) is worse than
   none: check it actually ran.
2. Implement. Keep the invariants (CLAUDE.md): no game or ROM data is ever
   committed, and every differential suite still matches. A suite that moves
   is a deliberate, called-out decision, never a check adjusted to pass.
3. **Gate on the exit code, never on grepped output:**

   ```bash
if .claude/scripts/check.sh > "$TMPDIR/check.log" 2>&1; then echo "GATE PASS"; else echo "GATE FAIL"; tail -40 "$TMPDIR/check.log"; fi
```

   `check.sh` runs what CI runs, plus the differential suites when the game
   and ROM are in `assets/` — and they are the gate that matters, because CI
   cannot run them. The tool shell is zsh: never pipe the gate into `tail` and
   read `$?`, since that's the pipe's status.
4. One commit per task, with a message that says what and why. Push, then tick
   the task in the issue's *Plan*.
5. **Watch CI to completion in the foreground and read EVERY job**, for the
   commit you just pushed (a stale run shows for a few seconds after a push):

   ```bash
   while gh pr checks <n> --json bucket -q '.[].bucket' | grep -q pending; do sleep 30; done
   gh pr checks <n> --json name,bucket -q '.[] | "\(.bucket)\t\(.name)"'
   ```

   A subagent is never woken by its own background task, so don't background
   the watch and end. **A flaky test is a bug**: reproduce it, root-cause it,
   and fix the cause. A re-run unblocks a PR; it doesn't end the flake.

## The PR

Open it early as a draft, linked to the ticket. **`Closes #NN` only if this PR
completes every task in the ticket's plan**; otherwise `Part of #NN`. A task
left for the maintainer (a settings change, a manual step) counts as open: a
`Closes` would shut the ticket on merge with that task still undone (starquake-recompiled#22).

**Review is the bottleneck**: the maintainer reviews alone, so the body is a
guide to reviewing, not a defence.

0. **No attribution line on a PR body or a commit message** — the maintainer
   asked for those to stay clean. Issues and comments carry it; these do not.
1. **`## Where to look`**: the two or three judgement calls the maintainer
   might disagree with, each naming its file. None? Say so in one line.
2. `---`, then *Mechanically verified — skip unless curious.* and ONE short
   paragraph: which gates passed and what was checked.

Comments follow the same rule: the answer goes in the **first line**. Keep
mechanical churn (regenerated files, formatting) in its own commit.

## Review the whole diff

Before the PR is handed over, read the change back as a reviewer would, all
of it against `main`, not just the last commit (starquake-recompiled#74):

```bash
git fetch origin && git diff origin/main...HEAD
```

The gates prove it compiles, lints and passes its tests. They cannot see:

- **Leftovers**: code, names, comments and docs from an earlier iteration, or
  from a design the maintainer changed along the way. A lint catches an unused
  function; it does not catch a comment describing behaviour that is gone.
- **The ticket**: each item in *Decisions*, and the approved mockup, against
  what the code actually does.
- **States and inputs**: what every key and button does in every state of
  the thing built; what crosses between threads; what a key or button still
  held does when a screen changes under it.
- **Tests** that assert the decisions, not the current implementation.
- **Fidelity**: whether anything could move a differential suite or the Z80
  corpus.

**Sort each finding into one of two kinds** (starquake-recompiled#74):

- **A defect**, with one right answer: a bug, leftover code or a stale
  comment, a name from an earlier iteration, a test that does not test what it
  says. **Fix it without asking**, in its own commit.
- **A judgement call**: anything that changes what the player sees or how it
  behaves, departs from the ticket's *Decisions* or the approved mockup,
  widens or narrows the scope, or touches fidelity to the original. **Do not
  fix it; ask.** When unsure which kind a finding is, it is a judgement call.

Ask by posting a review comment on the line it is about. It opens with the 🤖
attribution line, says what is wrong, gives the recommended fix, and ends
with the three words the maintainer can reply with:

```bash
gh api repos/zx-recompiled/robin-of-the-wood-recompiled/pulls/<n>/comments \
  -f commit_id="$(git rev-parse HEAD)" -f path=<file> -F line=<line> -f side=RIGHT \
  -f body="$(cat finding.md)"
```

| reply starts with | Claude |
|---|---|
| `fix` | fixes it as recommended (or as the reply amends), pushes, replies with the commit, and resolves the thread |
| `skip` | leaves it, replies to acknowledge, and resolves the thread |
| `ticket` | files it as a Backlog issue, replies with the link, and resolves the thread |

Any other reply is a question or an extra comment: answer it in the thread,
and act on it only when it asks for a change. Resolve a thread once it is
acted on and nothing is left to answer; **the ruleset blocks merging while any
conversation is unresolved**, so a thread left open holds the PR. Resolving
takes the thread's node id:

```bash
gh api graphql -f query='{ repository(owner:"zx-recompiled", name:"robin-of-the-wood-recompiled") {
  pullRequest(number:<n>) { reviewThreads(first:50) { nodes { id isResolved
    comments(first:1) { nodes { databaseId } } } } } } }'
gh api graphql -f query='mutation { resolveReviewThread(input:{threadId:"<id>"}) { thread { isResolved } } }'
``` Replies arrive through the board
monitor, which watches PR review comments. A finding with no line to anchor
to (something missing) goes on the file's first changed line, saying so.

The PR body gets a *Found in review* section: each defect fixed, with its
commit, and how many judgement calls are waiting as comments (or that there
were none of either). The card moves to `Your review` with those still open;
the maintainer's replies are part of the review.

## Finish

- Docs: update `README.md` / `CLAUDE.md` if anything they say changed. If the
  change touches what the differential suites prove, `README.md`'s *Verification* section
  says what the claim rests on and must stay true to it.
- **A mockup in the ticket?** Its image must reach `main` in this PR. Once
  merged, repoint the embed from `/raw/<branch>/` to `/raw/main/`, because
  merging deletes the branch and a branch embed 404s.
- `gh pr ready <n>`, watch CI green, then **STOP. Never merge.** The
  `ready to merge` label is the maintainer's; `merge-pr` lands it once they
  add it.
- **Re-read the ticket's plan before handing over.** Every box this PR covers
  is ticked. If any box is still open, including one that's the maintainer's,
  the PR body must say `Part of #NN`, not `Closes #NN`; fix it now.
- **Check the ticket's sub-issues too** before writing `Closes #NN`:
  `gh api repos/zx-recompiled/robin-of-the-wood-recompiled/issues/<n>/sub_issues -q '.[] | select(.state=="open") | .number'`.
  Any still open (other than ones this PR closes) means `Part of #NN`: PR starquake-recompiled#76
  closed starquake-recompiled#1, the guidance levels' parent, with four still open (starquake-recompiled#84). A PR for
  a sub-issue says `Closes` only for that sub-issue, never for its parent.
- Move the card to **`Your review`** (NOT `Your sign-off`, which is the
  pre-build gate) and post a Next-steps comment: *Next: review the PR and add
  `ready to merge`.* List any task still open after the merge in that comment,
  with whose it is.
