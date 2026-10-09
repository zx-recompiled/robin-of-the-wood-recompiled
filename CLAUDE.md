# robin-of-the-wood-recompiled

Robin of the Wood (Odin Computer Graphics), the ZX Spectrum 128K release,
reimplemented from scratch in Rust, running natively rather than under
emulation. **The repository contains no original code or data**: every
graphic, map, text and sound will be read at startup from the player's own
copy of the game, which is required at runtime.

It is made the way [starquake-recompiled](https://github.com/zx-recompiled/starquake-recompiled)
was made, and its crates, workflow and skills were copied from there
(`REUSED.md`). The rewrite will be verified against the original by
differential suites that run the original's routines in a reference Z80
interpreter (`crates/zx-runtime`) beside the rewritten code and compare the
result byte for byte.

**Where it stands:** the original boots from its tape in the 128K reference
machine, through the real ROM and its own loader, to the state every check
will start from (#3; `games/robin/tests/boot.rs`). The facts it needs are in
`games/robin/src/layout.rs` and `docs/re/robin.md`. The game reads its tape
(`robin::assets::read_game`, #5): the SHA-1 checked, then the eight banks
built from the tape alone, with no ROM. Rewritten and checked by the
differential suites (`tools/robin-verify`): the screen and printing (#24),
the map, drawn at all 320 locations by a tour (#28), the sprites (#32),
Robin's movement and controls (#34), the four characters on each row
(#38), Robin's actions and sounds (#36), the wanderer (#39), the
fighting (#40), and the rest of the main loop's routines (#41: the second
group, items, the fifth character, the trade, doorways and journeys, BREAK
and the game over), checked as far as play reaches them. The plan, in
order, is the board's Backlog.

## Commands

- `.claude/scripts/check.sh` is the pre-PR gate: build, tests, clippy, docs,
  the no-frontend build, dependency policy, and then the differential suites
  (`robin-verify`), which **fail the gate without the tape and `128.rom` in
  `assets/`**. **Gate on the exit code, never on grepped output.**
- `cargo run --release -p robin-verify -- assets` runs the differential
  suites alone, about three minutes on a 12-core machine: the calls are
  checked on every core (#45). It compares every rewritten routine against
  the original's real calls over 20,000 frames of play, then a tour of all
  320 locations, walking Robin off the screen at every move (#46), then a
  short game with each control method, one with Robin armed (#36), and
  one with things to pick up where he starts (#41). It prints each
  routine's cases and the instructions no call reached.
- `cargo test -p zx-runtime --test z80test --test fuse -- --nocapture` checks
  the interpreter against z80test (measured on a real Spectrum) and the Fuse
  Z80 corpus (1335 cases, for timing). Their files go in `assets/`.
- `cargo test -p zx-runtime --test machine128` checks the 128K machine's own
  behaviour: paging, the lock, the shadow screen, contention by bank, the AY.
- `cargo test -p zx-runtime --test hardware -- --nocapture` runs both machines
  against test programs measured on real ones (`minfo`, Fuse Test, Rak's
  Timing Test against a real +2's photographs, Butler's), booting the real
  ROMs and loading each program from its tape. Needs the files in `assets/`.
- `cargo run --release -p zx-recomp -- tools/re/robin.toml --listing target/re/robin.lst`
  traces the original as it plays and writes its disassembly, a section per
  page (`docs/re/robin.md`, *Making a listing*). Needs the tape and ROM in
  `assets/`. **A listing is never committed**: it is the original's code.
- The tool shell is zsh: never name a variable `status`, and run anything
  loop-shaped as a `bash` script.

## Invariants

- **No game or ROM data is ever committed.** That means what the game is
  loaded from: tapes, snapshots and ROMs, and graphics, maps, text or sound
  extracted from them as files. `assets/` is ignored except its README, and
  CI has a job that fails if anything slips through. This is what makes the
  project legal to publish; nothing is worth breaking it for.
  **Screenshots of the game are fine to commit** (the maintainer's call in
  starquake-recompiled#140): a picture of the screen in the README, the docs
  or a mockup is not data the game can be loaded from. Keep to the pictures a
  page needs, not a gallery of the game.
- **The game reads its data from the player's tape at startup.** The code
  knows only where each table lives and how to parse it. Exactly one dump is
  supported, pinned by SHA-1 (`assets/README.md`), and anything else is
  refused with a clear message.
- **Reverse-engineering notes are in our own words** (`docs/re/robin.md`):
  descriptions, addresses and layouts, never listings of the original's code.
  Each fact is marked **confirmed** (seen running the original in the
  reference machine), **provisional** (seen only in an unchecked emulator),
  **read** (understood from the disassembly) or **guess**.
- **The ROMs are for development only.** The reference machine needs them to
  run the original; the game never does.
- **The differential suites are the contract.** Every one must match. A
  change that moves one is a deliberate, called-out decision, never a check
  adjusted to make it pass.
- **The interpreter is not self-certified.** Everything else rests on it, so
  it is checked against outside references rather than against our own work:
  z80test, measured on hardware, for flags and registers, and the Fuse corpus
  for timing. All three z80test programs must pass, and the corpus must match
  in every case but the six `KNOWN_WRONG` ones in `tests/fuse.rs`. Most of
  MEMPTR is unconfirmed (starquake-recompiled#145). They pass on both the 48K
  and the 128K machine. The machines' timing is checked against test programs
  whose results were measured on real machines, never against another emulator
  (`tests/hardware.rs`): every known difference is listed by name, with its
  ticket, and the test checks exactly those differ (#14 the end of a contended
  line, #16 Butler's tests, the floating bus out of scope).
- **Fidelity first, and say so when it is not.** Where the rewrite cannot match
  the original exactly, the reason is written down (`README.md`, *Status*)
  rather than left to be discovered.
- **Nothing is offered upstream.** Third-party code (RustZX's `aym`, for the
  AY sound) comes from the maintainer's own copy in `zx-sidekick/zx-sidekick`,
  pinned to a commit; changes go there, never to the original project.

## How work lands

**The ticket is canonical.** Every conversation about a piece of work happens
in its GitHub issue. Chat is optional and the maintainer may not read it: an
answer given in chat is written back into the issue body before acting on it.

- **Claude reviews its own diff before handing a PR over**: the whole branch
  against `main`, for what the gates cannot see (leftovers from earlier
  iterations, behaviour against the ticket, input and state edge cases).
  Defects are fixed straight away and listed in the PR; judgement calls are a
  review comment on their line, left for the maintainer to answer `fix`,
  `skip` or `ticket` (`build-slice`).
- **Everything lands via a pull request** with an issue behind it, including
  chores and docs. One issue, one deliverable; a ticket that needs several PRs
  in different states is split into sub-issues. A PR says `Closes #NN` only
  when it completes every task in the ticket's plan, maintainer steps included;
  otherwise `Part of #NN`, and the ticket stays open. **A parent issue with a
  sub-issue still open is never closed by a PR**, whatever its own plan says,
  and a sub-issue's ticket or PR never mentions closing its parent: they say
  `Part of #NN` (in starquake-recompiled, #76 closed #1 with four levels
  still open, #84).
- **The board is the handoff baton**: the Status field of the "Robin of the
  Wood Recompiled" org Project
  (https://github.com/orgs/zx-recompiled/projects/2). Read and move it with
  `.claude/scripts/board.sh`.

  ```
  Backlog · Your input · Spec · Plan · Your sign-off · Build · Your review · Done
  ```

  **If a state says "your", it is the maintainer's gate** and work stops;
  `Spec`, `Plan` and `Build` are Claude's and proceed without re-asking.
  `Your input` (questions) can interrupt any stage. `Your sign-off` comes
  BEFORE a build (approve the spec, plan or mockup); `Your review` comes AFTER
  it (the PR is open, awaiting `ready to merge`). Cards move both ways and
  stages can be skipped: a bug or tweak goes straight to `Build`.

- **Approval** of a spec or plan is the maintainer dragging the card on, or a
  `go` / `approved` comment. Never proceed from plan to build without it.
- **A card dragged to `Spec` means "your call"**: first decide whether it needs
  a spec at all, and say so on the ticket. A bug or tweak goes on to `Build`.
- **The `Backlog` column's order is the priority.** Nothing leaves `Backlog`
  without the maintainer; "pick up the next one" means its top card.
- **Merging needs the `ready to merge` label** on the PR, re-read from the API
  at the moment of merging. Claude never adds it and never merges without it.
- **A position in the flow is a Status; a property of a ticket is a label**:
  `ready to merge`, `hold` (skip entirely), `needs: spec` / `needs: build`
  (the route, set when filing, with the reason in the body).
- **The body is the living spec; the comments are append-only history.** When
  a question is answered it moves into _Decisions_ and is deleted from _Open
  questions_. Every state change gets a NEW `> 🤖 **Next steps**` comment;
  never edit an old one.
- **Questions go in a copy-paste answer block**: a fenced block headed
  `# keep your pick, delete the rest`, one line per question, every line
  carrying a `(rec)`, ending with `notes =`. Posting one moves the ticket to
  `Your input` in the same step.
- **Visual work gets a mockup approved before the real UI is built**
  (`mockup` skill). On this project a mockup of anything the Spectrum draws is
  a real screenshot at 256×192, not an HTML sketch: the constraint is the
  design.

### Attribution: comments yes, commits and PR bodies no

`gh` acts as @starquake, so an unmarked Claude comment reads as the
maintainer's own answer — and the board monitor tells them apart by exactly
that prefix. So **every issue and comment Claude posts** opens with one of
these lines, posted via `--body-file`:

- `> 🤖 **Issue by Claude** (AI pair-programmer working with @starquake) — posted through @starquake's account.`
- `> 🤖 **Comment by Claude** (AI pair-programmer working with @starquake) — posted through @starquake's account.`

**Commit messages and pull request descriptions carry no attribution line**, by
the maintainer's standing instruction. Nothing reads those for provenance, so
nothing is lost.

The procedure behind each step lives in the skills: `work-the-board` (and its
`/board` alias), `design-slice`, `mockup`, `build-slice`, `merge-pr`,
`issue-comment-replies`.
