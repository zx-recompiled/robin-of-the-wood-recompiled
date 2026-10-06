# Reused from starquake-recompiled

This project copies its groundwork from
[starquake-recompiled](https://github.com/zx-recompiled/starquake-recompiled)
rather than depending on it: an API designed around one game fits that game,
and a shared kit is better extracted once a second game has shown which parts
are really generic. Until then, this file records what was copied, from
which commit, and what was changed on the way in.

Source commit: `84237d7` (starquake-recompiled#144, the interpreter checked
against z80test), copied 2026-10-06.

| Copied | Changed on the way in |
|---|---|
| `crates/zx-core` | One comment qualified as starquake-recompiled's. Since diverged (#2): a machine's timing as a value (`timing::Timing`), contention that asks the machine, and a `Model` in `MachineState`. Then (#12): a `.szx` reader, with `miniz_oxide`. |
| `crates/zx-runtime`, with `tests/fuse.rs` and `tests/z80test.rs` | Nothing. Since diverged (#2): memory as four slots over ROM and RAM pages, the 128K machine, the AY's registers, and z80test on both machines. The 48K machine's results are unchanged. Then (#12): a tape feeder and a screen reader, and two interrupt faults fixed (no interrupt after a lone `DD`/`FD` prefix; the pulse ends at its length), which starquake-recompiled's copy still has. |
| `crates/zx-recomp` | One comment qualified as starquake-recompiled's. Since diverged (#4): code is traced, analysed and listed by where it is (a page and an offset), so the same address in two 128K banks is two pieces of code; a configuration can name a 128K, booted from its tape and ROM; `banked_calls` decodes a banked-call routine's inline target; the listing has a section per page. A 48K is numbered as before. |
| `.claude/skills/*` (`board`, `build-slice`, `design-slice`, `issue-comment-replies`, `merge-pr`, `mockup`, `work-the-board`) | This repository and its board; Starquake's issue numbers written as `starquake-recompiled#NN`; Starquake's code examples swapped for this repository's; no fixed suite count. |
| `.claude/scripts/check.sh` | Builds `robin`; says loudly that there are no differential suites yet, in place of running `sq-verify`. |
| `.claude/scripts/board.sh` | This repository and its board. |
| `.github/ISSUE_TEMPLATE/spec.md` | This repository; no fixed suite count. |
| `.github/workflows/ci.yml` | Builds `robin`. Without the Intel Mac job and the frontend's Linux libraries, which come back with the frontend. |
| `Cargo.toml` (workspace), `deny.toml`, `about.toml`, `rust-toolchain.toml`, `LICENSE-MIT`, `LICENSE-APACHE`, `.gitignore` | Workspace members; the toolchain note names the gate. |
| `about.hbs`, `CONTRIBUTING.md` | Rewritten for Robin. |

Not copied, and why:

- `games/starquake`, `tools/sq-verify`, `tools/re/starquake.toml`, `docs/`:
  Starquake's own game, checks and notes.
- `.github/workflows/release.yml` and the Inter fonts: they come with
  Robin's frontend, when there is a program to release.
- `.git-blame-ignore-revs`: names Starquake's own commits.

Written new: `README.md`, `CLAUDE.md` (its *How work lands* section is
Starquake's, adapted), `assets/README.md`, `docs/re/robin.md` and the
`games/robin` stub.
