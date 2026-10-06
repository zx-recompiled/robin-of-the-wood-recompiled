---
name: design-slice
description: >
  Use whenever a ticket needs its spec or plan: "let's design X", "spec out
  #NN", "pick up #NN" when #NN has no approved plan yet, or a ticket sitting in
  `Spec` or `Plan` on the board. Runs the design-first workflow: the spec and
  plan are written IN the GitHub issue (spec template), decisions are settled
  with the maintainer there through a copy-paste answer block, and building
  only starts after the maintainer's explicit OK. Trigger even if the user
  doesn't say "skill".
---

You design a change **in its GitHub issue**: the issue is the single design
record, and the maintainer may never read chat. The one rule that matters
most: **never auto-proceed from plan to implementation.** The maintainer's OK
(the card dragged to `Build`, or a `go` / `approved` comment) is the build
signal, and it must be explicit.

Move the card at each pause with `.claude/scripts/board.sh state <n> "<state>"`.
`Spec` and `Plan` are yours; `Your input` and `Your sign-off` are the
maintainer's.

## Step 1: the issue

- **First, choose the route.** A card in `Spec` means "your call, go", not
  "write a spec": decide how much process it needs, and say so on the ticket
  with the reason (starquake-recompiled#26).
  - **A bug or tweak**, with no design question: comment *"Routing: straight to
    Build — …"*, move it to `Build`, and hand it to `build-slice`. No spec.
  - **A small decision**: a short spec with its answer block; the plan is a few
    lines in the same body once it's answered.
  - **A feature**: the full route below.

  The maintainer overrides by dragging the card back.
- **No issue yet?** Create one from `.github/ISSUE_TEMPLATE/spec.md`. Read that
  file and fill its sections; `gh issue create --body-file` bypasses templates
  entirely, so nothing fails when the structure goes missing. The body's first
  line is the attribution header:
  `> 🤖 **Issue by Claude** (AI pair-programmer working with @starquake) — posted through @starquake's account.`
- **A sub-issue** (filed under a parent, or split out of one) names its parent
  as `Part of #NN` and never with `Closes`, `Fixes` or `Resolves`; nor does
  its plan ask its PR to close the parent. A parent's own plan, while any
  sub-issue is open, ends in `Part of #NN` too: starquake-recompiled#76 closed starquake-recompiled#1 with four
  guidance levels still open (starquake-recompiled#84).
- **The issue exists but is free-form?** Restructure its body into the
  template's sections, keeping everything true that's already there.
- **The BODY is the living spec; the COMMENTS are the history.** Opposite
  editing rules:
  - The **body** is always current: rewrite it freely. When a question is
    answered, MOVE it into *Decisions* and DELETE it from *Open questions*. A
    body that still asks a settled question is the bug.
  - The **comments** are append-only: never edit one; post a new
    `> 🤖 **Next steps**` comment each time the state changes.
- Read the code before writing: name real symbols
  (`crates/zx-runtime/src/interp.rs`, `Zx::block_repeat_flags`,
  `bus::cycles`), not "the interpreter".
- Set `Spec` while you're writing it, so the board shows it's your turn.

## Step 2: the spec

- **Goal**: what ships, plus the one-line reason.
- **Decisions**: numbered, each with its why. Anything unsettled is a question
  TO the maintainer. Never decide design direction yourself.
- **Fidelity**: say whether the change can move any of the differential
  suites or the Z80 conformance tests (z80test and the Fuse corpus). If it
  can, the ticket says why that is right before the work starts — a suite
  is never adjusted to make a change pass. Say too whether it needs the game
  or ROM in `assets/`, which CI has not got.
- **Open questions end with a copy-paste answer block.** One line per
  question, the options inline, and **every line carrying a `(rec)`**:

  ~~~
  ```
  # keep your pick, delete the rest
  Q1 training mode freezes: bar 0 only (rec) / all three bars
  Q2 toggle lives in: the menu (rec) / a command-line flag
  notes =
  ```
  ~~~

  The recommendation belongs **in the block**, not in prose under it: a bare
  menu makes the maintainer reconstruct reasoning you already did. No view?
  Write `discuss (rec)` on that line: "no recommendation" is real information.
  Before posting, re-read the prose against the block: **a question missing
  from the block is effectively not asked.**

  Mirror the block in the Next-steps comment, so it can be answered from the
  thread.
- **Answers in chat are allowed; leaving them there is not.** When the
  maintainer answers conversationally, write the answers into the BODY
  (*Decisions*, dated, with a verbatim quote wherever the wording carries
  reasoning) and post a Next-steps comment saying so **before acting on
  them**, so the record survives a session that ends unexpectedly. A fix to a
  missing or wrong block is a NEW comment, never an edit.
- **Mockup**: if the change's value is how it looks, make the mockup now
  (`mockup` skill) and embed it in the *Mockup* section. Approving the
  screenshot is part of the spec OK.
- **Hand off**: posting the block moves the ticket to `Your input` in the same
  step. Nothing open? Skip straight to the plan.

## Step 3: settle, then plan

Fill the plan only once the decisions are settled, and set `Plan` while you
write it. Tasks go in landing order, each one green commit
(`.claude/scripts/check.sh`); failing tests first where practical; the last
task updates `README.md` / `CLAUDE.md` if anything they say changed.

## Step 4: STOP

Set `Your sign-off`, post a Next-steps comment (*Next: drag the card to `Build`
or comment `go`*), and **end there**. The build belongs to `build-slice` and
starts only on the maintainer's go.
