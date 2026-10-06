---
name: mockup
description: >
  Use whenever visual or looks-driven work needs its pre-approval mockup: "make
  a mockup of X", "show me what it would look like", "design the screen
  first", or from inside a spec whose value is how it LOOKS (a new screen, a
  layout change, colours, typography, animation). Produces a screenshot,
  commits it under docs/mockups/ on the work branch, and embeds it in the
  ticket. The maintainer
  approves the picture BEFORE any real UI is built. Trigger before writing any
  UI code for looks-driven work, even if nobody asked for a mockup.
---

Visual work gets a mockup approved **before** the real UI exists. You produce
the mockup, the screenshot and the embed; the maintainer's yes to the picture
is part of the spec OK.

## Step 1: build it

Pick whichever shows the design fastest and most faithfully:

- **A real screenshot**, which this project will produce without a window
  once the frontend exists (step 8 of the plan; until then, a screenshot from
  the reference machine):

  ```bash
  cargo run --release --all-features -p robin -- assets/<the tape> \
    --headless 4000 docs/mockups/shots
  ```

  Anything drawn on the Spectrum's screen should be mocked up this way rather
  than in HTML, because the constraint *is* the design: 256×192, 32×24 colour
  cells, two colours per cell, and the game's own font. A mockup that ignores
  that is not a mockup of this game.

  **Screenshots of the game are fine to commit**, original graphics and all
  (the maintainer's call). The no-game-data invariant is about the files the
  game is loaded from: tapes, snapshots, ROMs, and graphics, maps or text
  extracted from them as files, which CI's guard job looks for. A picture of
  the screen for a design review is not one of those. Keep to the frames the
  design needs, not a gallery of the game.
- **An HTML mockup** in the scratchpad, only for chrome that lives *outside*
  the Spectrum's screen — a host window, a map panel beside the picture, a
  settings sheet. Say which it is, so nobody reads a host-side sketch as
  something the 1985 machine would draw.

**Look at the PNG yourself before posting it**, to catch a blank or clipped
render.

## Step 2: commit the image

`docs/mockups/<YYYY-MM-DD>-<name>.png`, on the **work branch** (never straight
to `main`). The repo is the image host, since GitHub has no upload API for
issue attachments.

## Step 3: embed with exactly this URL form

```markdown
![mockup](https://github.com/zx-recompiled/robin-of-the-wood-recompiled/raw/<branch>/docs/mockups/<file>.png)
```

This repo is public, so `raw.githubusercontent.com` would also render. Use the
`github.com/…/raw/…` form anyway: it is the one that keeps working if the repo
is ever made private again, and `github.com/…/blob/…` is a click-through link
rather than an embed.

Put it in the ticket's *Mockup* section. If you post it in a comment instead,
the comment opens with the 🤖 attribution line.

**The embed must end up pointing at `main`.** A `/raw/<branch>/` embed dies
when the branch is deleted on merge, silently and retroactively. The PR that
merges the image repoints the embed to `/raw/main/…` (`build-slice`, Finish).
Not a commit SHA: PRs are squash-merged, so the branch's commits aren't in
`main`'s history.

## Step 4: STOP for approval

Move the card to `Your sign-off` and post a Next-steps comment asking for a
yes, or for changes via an answer block. No real UI code before that. To
iterate, re-render to the same filename on the same branch: the embed then
shows the new version.
