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
| `.claude/scripts/check.sh` | Builds `robin`; says loudly that there are no differential suites yet, in place of running `sq-verify`. Since diverged (#24): runs `robin-verify`, and fails without the tape and ROM. |
| `.claude/scripts/board.sh` | This repository and its board. |
| `.github/ISSUE_TEMPLATE/spec.md` | This repository; no fixed suite count. |
| `.github/workflows/ci.yml` | Builds `robin`. Without the Intel Mac job and the frontend's Linux libraries at first; they came back with #68. |
| `Cargo.toml` (workspace), `deny.toml`, `about.toml`, `rust-toolchain.toml`, `LICENSE-MIT`, `LICENSE-APACHE`, `.gitignore` | Workspace members; the toolchain note names the gate. |
| `about.hbs`, `CONTRIBUTING.md` | Rewritten for Robin. |
| `games/starquake/src/frontend/overlay.rs` and `overlay.wgsl`, the panel's layout in `panel.rs`, and the window's overlay pass in `video.rs`, at `84237d7` (#95) | The overlay as it was. The panel is Robin's own, with aids rather than guidance levels, and the picker is a list of switches rather than a level. Its keys are F1 and Tab, since Escape is BREAK here. `text.rs` gets back the drawing primitives the overlay uses (`clear_transparent`, `shade`). |
| `games/starquake/src/frontend/prompt.rs`, `text.rs`, `tape.rs` and `games/starquake/fonts/` (Inter, OFL), at `84237d7` (#70) | Robin's title, page (World of Spectrum's item 4177) and checks. The tape is found by content, any `.tzx` or a zip holding one, where Starquake's went by its names. It's kept as `robin-of-the-wood.tzx`. The drawing primitives only the overlay used are left out. |
| `games/starquake/src/frontend/gamepad.rs`, `.github/workflows/release.yml`, and CI's Intel Mac job and system libraries, at `84237d7` (#68) | The gamepad read only as the Kempston joystick (in play, since #84, through whichever method the menu chose), any face button firing and Start pressing 0, without the picker, the layouts or the held-back presses. The release for `robin`, without the player's guide or a font, which Robin doesn't have yet, and the audio library left out of the packages until there was sound (#67, now in). |
| `games/starquake/src/frontend/audio.rs`, at `84237d7` (#67) | The beeper from the 128K's writes to `0xFE` at their times (`robin::io::Io::ula_writes`), EAR only, at the 128K's clock, mixed with the AY through `aym` (zx-sidekick's `crates/aym`, pinned to a commit) from its timed writes (`Io::ay_writes`). A sound past its frame's end is carried into the frames that follow, where Starquake's were played over the frames they took at once. A queue of a second, and a WAV writer for the headless run. |
| `games/starquake/src/frontend/input.rs`, the window in `frontend/video.rs`, and `frontend/headless.rs`, at `84237d7` (#66) | Into `games/robin/src/frontend/`. The keyboard's map as it was, with Escape for BREAK and without Backspace and the cursor keys' digits. The window without the guidance panel, the overlay or the tape prompt, on one thread: it steps a `robin::session::Session` a frame at a time, where Starquake's game ran on its own thread. The picture's drawing moved into the library (`robin::picture`), the same border and palette. The headless run without its effect tallies. |

Not copied, and why:

- `games/starquake`, `tools/sq-verify`, `tools/re/starquake.toml`, `docs/`:
  Starquake's own game, checks and notes.
- `.git-blame-ignore-revs`: names Starquake's own commits.

Written new: `README.md`, `CLAUDE.md` (its *How work lands* section is
Starquake's, adapted), `assets/README.md`, `docs/re/robin.md` and the
`games/robin` stub.
