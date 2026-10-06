---
name: work-the-board
description: >
  Use when the maintainer says "work the board": do ONE triage-and-advance pass
  over the repo's open issues and pull requests (plus recently closed ones, for
  late comments), moving each to its next step in the ticket workflow. Reply to
  comments, draft or refine specs and plans, build slices the maintainer has
  already approved, and merge PRs that carry `ready to merge`. It stops at every
  maintainer gate. "work the board every 30m" / "…in a loop" runs the same pass
  on a schedule (see Loop form). Trigger on the phrase even without "skill".
  "stop the board" ends the loop.
---

You run the issue tracker forward so the maintainer can drive everything
through tickets and the board. The ticket is canonical (CLAUDE.md): the
maintainer may never read chat. A pass reads the board, does every
**work-state** step it can, carries out actions the maintainer has already
authorised (a go signal, a `ready to merge` label), and **stops at every gate
that is theirs**.

Read the board with `.claude/scripts/board.sh list "<state>"` and move a card
with `.claude/scripts/board.sh state <n> "<state>"`.

**Two kinds of state, and the name tells you which:**

- **Gate states** (`Your input`, `Your sign-off`, `Your review`): work *stops*.
  The maintainer decides. You may ask; you never answer for them.
- **Work states** (`Spec`, `Plan`, `Build`): work *proceeds*. The state is the
  instruction, and acting on it needs no further permission.

**If it says "your", it is a gate.**

**Do not re-ask permission on a work state, however big or risky it looks.**
A load-bearing decision inside a `Build` is made and **surfaced in the PR** for
review; the PR review plus `ready to merge` is the sign-off. Converting a work
state back into "shall I start?" re-invents a gate the maintainer already
opened. (mediumrogue, 2026-07-21: a large `Build` was held with "kick it off
now, or park it?"; the maintainer asked *"why don't you pick up the needs build
now?"*)

**But a work state is not a promise that the ticket is workable.** When a
ticket lacks the information to do the work (an empty body, a symptom with no
reproduction), do every bit of investigation that doesn't need the answer, then
send it back to `Your input` with **specific** questions and your findings
attached. "Shall I start?" re-invents a gate; "here is what I found, and here
are the two facts only you have" is the work.

**Never ship a fix on a reading-only diagnosis when the reproduction fails.**
Reading code tells you what CAN happen, not what DID. Publish the mechanism you
found and let the maintainer confirm the shape first.

## The autonomy contract: do not cross

- **Never decide design direction.** Open questions go TO the maintainer
  (`Your input`).
- **Never grant `ready to merge`, and never merge without it.** Re-read the
  label from the API at the moment you merge, never from a notification: a
  label can be added and withdrawn within a minute. The same goes for CI: green
  before a push is not green after it.
- **Build only what's authorised**: a plan the maintainer approved (`Build`, or
  a `go` / `approved` comment on a `Your sign-off` ticket), or a **bug** or
  **tweak** routed `needs: build`. Never build a `Your input` or `Your sign-off`
  ticket.
- **Skip anything labelled `hold`** entirely: no comment, no build, no merge.
- **Every issue and comment you post opens with the 🤖 attribution line**
  (CLAUDE.md), posted with `--body-file`. The monitor tells your comments from
  the maintainer's by that prefix, so an unmarked comment reads as their
  answer. **Commit messages and PR bodies carry no such line** — the
  maintainer asked for those to stay clean, and nothing reads them for
  provenance.
- **At most ONE build per pass** (see the cap).
- **Sweep for closed issues not in `Done`.** `Item closed → Done` is a GitHub
  automation, and when it breaks nothing announces it:

  ```bash
  gh api graphql -f query='{ organization(login:"zx-recompiled"){ projectV2(number:2){ items(first:100){ nodes{
    content{ ... on Issue { number state } }
    fieldValueByName(name:"Status"){ ... on ProjectV2ItemFieldSingleSelectValue { name } }
  }}}}}' --jq '.data.organization.projectV2.items.nodes[]
    | select(.content.number != null) | select(.content.state=="CLOSED")
    | select(.fieldValueByName.name != "Done") | .content.number'
  ```

  Move what it returns to `Done`. If it returns anything, say so: an automation
  is off, and fixing it is the maintainer's job (Project → ⋯ → Workflows).

## The pass

**A red `main` comes before all of it.** After any merge, yours or the
maintainer's, confirm `main` still builds:

```bash
gh run list -R "$R" --branch main --workflow CI --limit 1 --json conclusion,headSha
```

Two pull requests can each be green and still break `main` together, because
the ruleset does not require a branch to be up to date before merging: a PR's
checks passed against the base as it was when they ran, not as it is at merge.
Nothing else notices this.

**Fixing a red `main` is authorised work, not a question.** The change that
broke it is known, the fix is not a design decision, and every branch cut
while it is red inherits the breakage. Fix it, say what it was, and carry on.

**Standing authorised work goes first.** Read every work lane (`Build`, then
`Plan`, then `Spec`) and work from there. A spec waiting to be written is
standing work exactly as a build is. Work does not earn priority by being new;
the only pre-emptions are a red `main`, a PR carrying `ready to merge`, and a
direct maintainer request, and they are named as exceptions.

0. **Reconcile before anything else.** Never trust what you remember, or what
   a summary says, about whether a pull request is still open. The maintainer
   merges, and a pass that assumes otherwise works on a branch that has
   already landed.

   ```bash
   gh pr list -R "$R" --state merged --limit 10 --json number,title,mergedAt,mergeCommit
   git fetch origin --quiet && git log --oneline -5 origin/main
   ```

   For anything merged since the last pass: check `main` is still good (below),
   close out its ticket and card, and rebase any open branch that was cut
   before it.

1. **Enumerate.** `gh issue list --state open`, `gh pr list --state open`, and
   a recently-closed sweep for comments that landed after close. Drop anything
   labelled `hold`. **Re-read state from the API**, not from memory: a PR that
   was open last turn may not be now.

   **Read the comments, not just the states.** For every ticket at a gate,
   fetch its comments and look for the maintainer's answer block or go signal
   *after your last comment*. Scope that read by your last comment, never by a
   wall-clock window. An answer is a comment that does NOT start with `> 🤖`.

   **Read a ticket before answering anything about it**, including your own
   earlier comments. After a context break especially: a summary carries
   decisions, not what was already posted, so re-deriving and re-posting is the
   failure. If duplication already happened, own it in a NEW comment; never
   edit or delete the old one.
2. **Classify and act**, with **merges before builds** (work built before a
   pending merge lands on a stale base):

   1. cheap advancement (replies, labels, specs, plans, reminders)
   2. every PR carrying `ready to merge`: merge them all (`merge-pr`), then
      re-pull
   3. then the pass's one build, branched off the freshly merged `main`

   Hand each item to the skill that owns that step:

| State | Action | Owner |
|---|---|---|
| unanswered comment (issue/PR) | reply: factual auto-post, substantive draft for OK | `issue-comment-replies` |
| new issue, no `needs:` label | triage: feature → spec + questions → `Your input`; bug/tweak → `needs: build` | `design-slice` |
| **bug** routed `needs: build` | reproduce → root-cause → **green PR** | debug → PR |
| `Spec` | **choose the route first** (see Routing): a bug or tweak → comment the route → `Build`; otherwise draft the spec + its open questions → `Your input` (or `Plan` if nothing is open) | `design-slice` |
| `Your input` | **stop**, UNLESS a new maintainer comment answers the block → write the answers into the body → `Plan` | `design-slice` |
| `Plan` | write the plan (and mockup) → `Your sign-off` | `design-slice`, `mockup` |
| `Your sign-off` | **stop**, UNLESS the maintainer signalled go (a `go`/`approved` comment, OR moved it to `Build`) → build | `build-slice` |
| `Build` | build the approved slice → green PR → `Your review` | `build-slice` |
| `Your review` | **stop**: only `ready to merge` moves it. Do keep the PR mergeable: CI green, rebased if behind, and no thread left open that Claude has acted on (the ruleset refuses to merge with one). | `merge-pr` |
| reply to one of Claude's review comments | `fix`: fix, push, reply with the commit, **resolve the thread**; `skip`: acknowledge, resolve; `ticket`: file a Backlog issue, reply with the link, resolve; anything else: answer in the thread (starquake-recompiled#77) | `build-slice` |
| PR with new maintainer comments | address them, re-push | rework |
| PR carrying `ready to merge` | **merge it** (label + green CI + title + squash) | `merge-pr` |

3. **Post a Next-steps comment on every ticket whose state you moved** (below).
4. **The build cap: at most one build per pass, and ZERO is a valid pass.**
   Replies, advancement and merges are unlimited; do only the single
   highest-priority build (an approved slice, else a bug fix, else a tweak).
   If nothing is authorised, build nothing and say so. Never manufacture work
   to fill the slot.
5. **Looks-driven work gets its mockup first** (`mockup`). Drafting a spec or
   mockup is safe autonomous work, since it ends at a gate. Never defer it as
   "needs supervision".
6. **A ticket you FILE gets the same treatment as one you find, in the same
   action**: a Status naming whose turn it is, a `needs:` label with the
   routing reason in the body, and a Next-steps comment. A filed ticket with no
   next step is close to not having been filed.

## Routing a ticket

Every ticket you file states its route, with the reason, and carries the label:

```
Routing: straight to Build — a tweak, no unexamined assumption.
Routing: Spec first — the scoring change affects every platform's contract.
```

- **`needs: build`**: a bug, or a tweak (a number, a default, copy, a build
  script). Straight to a PR.
- **`needs: spec`**: a feature or change with a design decision whose
  assumptions want questioning first.

The maintainer disagrees by dragging the card elsewhere, and that is the
override. **Never add a Status option to carry a property**: reordering a
single-select replaces every option and clears every card's value.

**A card the maintainer drags to `Spec` means "your call, go"** (starquake-recompiled#26). The
first step there is choosing the route, not writing a spec: comment the route
with its reason, and move a bug or tweak straight on to `Build` (and build it,
within the one-build cap). Only a real design question gets a spec.

## Which backlog ticket is next

**The order of the `Backlog` column is the priority**, top first; the
maintainer ranks it by dragging. A pass never pulls from `Backlog` on its own:
a card leaves it only when the maintainer moves it, or asks in chat or a
comment to "pick up the next one", which means the **top** card. Read the
order through the API; the position sort is what the board shows:

```bash
gh api graphql -f query='{ organization(login:"zx-recompiled"){ projectV2(number:2){
  items(first:100, orderBy:{field:POSITION, direction:ASC}){ nodes{
    content{ ... on Issue { number title } }
    fieldValueByName(name:"Status"){ ... on ProjectV2ItemFieldSingleSelectValue { name } } } } } } }' \
  --jq '[.data.organization.projectV2.items.nodes[] | select(.fieldValueByName.name=="Backlog")][0].content'
```

Picking it up follows its route label: `needs: build` goes to `Build`,
anything else to `Spec` (where the route is decided as above). Post a
Next-steps comment saying it was picked up and by whose request.

## The Next-steps comment: a new one whenever the state moves

Every open ticket carries a comment headed `> 🤖 **Next steps**`, under the
attribution line, stating where the ticket is and what moves it.

**Post a NEW comment each time the state changes; never edit the previous
one.** The thread is the ticket's history. Nothing changed? Post nothing.
Write it as a reply ("folded your answers in; the plan's in the body; one thing
I didn't decide for you"), not a dashboard.

**Posting an answer block IS a `Your input` move.** Set the state in the same
step, never later.

Content by state (a state line, then "Next:" naming the action and who takes
it):

- `Your input`: *Next: paste the answer block back filled in (or answer in
  chat and I'll write it back).* **Always include the block**: fenced, headed
  `# keep your pick, delete the rest`, one line per question with its options
  and a `(rec)`, then `notes =`.
- `Your sign-off`: *Next: drag the card to `Build` or comment `go`; a pass
  builds it into a PR.*
- `Spec` / `Plan`: *Next: Claude drafts it and hands back; nothing needed from
  you.*
- `Build`: *Next: Claude builds a PR; then add `ready to merge` once you've
  reviewed it.*
- `Your review`: *Next: review the PR and add `ready to merge`; a pass merges
  it.*
- `Backlog`: blocked (name the blocker), a record (no action), or waiting for a
  go (say what the go is).

## Reporting

End the pass with: **what moved** (and to what state), **what you built or
merged**, and above all **what's now waiting on the maintainer** (the gate
queue). Report what you chose not to do as plainly as what you did: a build
declined for lack of authorisation, a decision refused, a flake you couldn't
reproduce. In a loop, this becomes a push notification only when something
needs them.

## Loop form

"work the board every `<interval>`" / "…in a loop" runs this same pass on a
schedule (`/board` is the short form). Same contract, same one-build cap.

### Prefer a Monitor over polling

A persistent Monitor runs the poll in the shell, outside the agent's context,
and only a printed line wakes the agent: quiet minutes cost nothing. Arm it at
the start of a loop session (check it isn't already running first). What it
watches is derived from **what the pass acts on**; every omission has cost a
real miss on mediumrogue.

| signal | why the pass cares |
|---|---|
| issue/PR conversation comments not starting `> 🤖` | the maintainer's answers and go signals |
| PR review comments (a different endpoint) | inline diff feedback |
| `ready to merge` added | the only merge authorisation |
| `hold` added / lifted | an override that stops or releases work |
| board Status transitions | a move to `Build` authorises a build |
| `main` going red | pre-empts everything |
| `main`'s head moving | something merged, and nobody is going to say so |
| standing work lanes (level-triggered) | authorised work that is merely waiting |

```bash
# pipefail is LOAD-BEARING: every snapshot ends in `| sort`, and without it a
# failed `gh` returns 0 with no output, so every guard silently passes.
set -o pipefail
R=zx-recompiled/robin-of-the-wood-recompiled
since=$(date -u +%Y-%m-%dT%H:%M:%SZ)
SELF="${BOARD_SELF_SET_FILE:-${TMPDIR:-/tmp}/robin-board-selfset}"
GQ='{ organization(login:"zx-recompiled"){ projectV2(number:2){ items(first:100){ nodes{
  content{ ... on Issue { number } }
  fieldValueByName(name:"Status"){ ... on ProjectV2ItemFieldSingleSelectValue { name } }
}}}}}'
# No `|| true` on a snapshot: a failed call must FAIL so the diff is skipped.
snap_board(){ gh api graphql -f query="$GQ" --jq '.data.organization.projectV2.items.nodes[]
  | select(.content.number != null) | "\(.content.number)|\(.fieldValueByName.name // "none")"' 2>/dev/null | sort -n; }
snap_label(){ gh pr list -R $R --state open --label "$1" --json number -q '.[].number' 2>/dev/null | sort; }
snap_hold(){ gh issue list -R $R --state open --label hold --json number -q '.[].number' 2>/dev/null | sort; }
snap_main(){ gh run list -R $R --branch main --workflow CI --limit 1 \
  --json conclusion -q '.[0].conclusion // "none"' 2>/dev/null || echo none; }
# `main` moving means something merged, which is the signal a pass most often
# used to get by being told.
snap_head(){ gh api repos/$R/commits/main --jq '.sha' 2>/dev/null; }

# Guard the initialisation too: a failed first call would start from a lie.
until prev_s=$(snap_board) && [ -n "$prev_s" ]; do sleep 30; done
prev_l=$(snap_label "ready to merge"); prev_h=$(snap_hold); prev_main=$(snap_main)
prev_head=$(snap_head); ticks=0
while true; do
  sleep 60
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)

  gh api "repos/$R/issues/comments?since=$since&per_page=30" \
    --jq '.[] | select((.body | startswith("> 🤖")) | not)
          | "COMMENT on #\(.issue_url | split("/") | last): \(.body | gsub("\n"; " ") | .[0:120])"' 2>/dev/null || true
  gh api "repos/$R/pulls/comments?since=$since&per_page=30" \
    --jq '.[] | select((.body | startswith("> 🤖")) | not)
          | "REVIEW COMMENT on #\(.pull_request_url | split("/") | last) \(.path): \(.body | gsub("\n"; " ") | .[0:100])"' 2>/dev/null || true

  # A set that was non-empty and is now empty is an outage until proven otherwise.
  if cur=$(snap_label "ready to merge") && { [ -n "$cur" ] || [ -z "$prev_l" ]; }; then
    comm -13 <(echo "$prev_l") <(echo "$cur") | grep -E '^[0-9]+$' | sed 's/^/READY TO MERGE: PR #/' || true
    prev_l=$cur
  fi
  if cur=$(snap_hold) && { [ -n "$cur" ] || [ -z "$prev_h" ]; }; then
    comm -13 <(echo "$prev_h") <(echo "$cur") | grep -E '^[0-9]+$' | sed 's/^/HOLD ADDED: #/' || true
    comm -23 <(echo "$prev_h") <(echo "$cur") | grep -E '^[0-9]+$' | sed 's/^/HOLD LIFTED: #/' || true
    prev_h=$cur
  fi
  cur=$(snap_main)
  [ "$cur" != "$prev_main" ] && [ "$cur" = "failure" ] && echo "MAIN IS RED: CI failed on main"
  prev_main=$cur

  # An empty result is an outage, not an empty branch, so only act on a real sha.
  if cur=$(snap_head) && [ -n "$cur" ]; then
    [ "$cur" != "$prev_head" ] && echo "MAIN MOVED to ${cur:0:8}: something merged, reconcile and check it builds"
    prev_head=$cur
  fi

  # Board Status, every transition. Moves board.sh made are consumed and skipped.
  if ! cur_s=$(snap_board) || { [ -z "$cur_s" ] && [ -n "$prev_s" ]; }; then since=$now; continue; fi
  printf '%s\n' "$cur_s" | while IFS='|' read -r n st; do
    [ -z "$n" ] && continue
    was=$(printf '%s\n' "$prev_s" | awk -F'|' -v k="$n" '$1==k{print $2}')
    [ "$was" = "$st" ] && continue
    if [ -f "$SELF" ] && grep -qxF "$n|$st" "$SELF"; then
      grep -vxF "$n|$st" "$SELF" > "$SELF.tmp" || true; mv "$SELF.tmp" "$SELF"; continue
    fi
    if [ -z "$was" ]; then echo "BOARD: #$n added as '$st'"; else echo "BOARD: #$n moved $was -> $st"; fi
  done
  prev_s=$cur_s

  # Level-triggered heartbeat: edge signals can't see work that is only waiting.
  ticks=$((ticks + 1))
  if [ $((ticks % 30)) -eq 0 ]; then
    waiting=""
    for lane in Build Plan Spec; do
      ids=$(printf '%s\n' "$cur_s" | awk -F'|' -v L="$lane" '$2==L{printf "#%s ", $1}')
      [ -n "$ids" ] && waiting="$waiting$lane: $ids"
    done
    [ -n "$waiting" ] && echo "STANDING work, outranks anything newly arrived — $waiting"
  fi
  since=$now
done
```

Why it's shaped like this:

- **Use the targeted GraphQL query, never `gh project item-list`, in a loop.**
  The targeted query costs 1 point; `item-list` costs about 100, and at a 60 s
  poll that exceeds the 5,000-points-an-hour GraphQL budget.
- **A failed poll is not an empty board.** Keep `|| true` on output that is
  emitted, and drop it from snapshots that are compared.
- **Never report your own board writes.** `board.sh state` records each write,
  and the monitor consumes one matching entry per transition. Consuming, rather
  than just matching, keeps a later genuine move to the same state visible.

With a Monitor armed, the scheduled tick is insurance only: about an hour.

### Pacing without a Monitor

In dynamic `/loop` mode, pick the next delay from what this pass found:

| This pass… | Next delay |
|---|---|
| merged, built, or is watching CI it pushed | ~5 min |
| folded in answers, replied, advanced a ticket | ~10 min |
| found new maintainer comments but nothing to do yet | ~15 min |
| found nothing (1st, 2nd, 3rd+ quiet pass) | ~20, ~40, 60 min |

Any activity resets to the top. After three quiet passes in a row, say so and
offer to stop.
