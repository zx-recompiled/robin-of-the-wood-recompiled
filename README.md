# Robin of the Wood, recompiled

Robin of the Wood, the ZX Spectrum 128K release (Odin Computer Graphics, 1986), rewritten from scratch in Rust to run natively: no emulator and no Z80 at runtime.

The original is by Steve Wetheril, Paul Salmon and Fred Gray, as credited in the dump's archive information.

**This project is not affiliated with or endorsed by the rights holders of Robin of the Wood. You need your own copy of the game:** the program will contain no part of it, and will read the graphics, maps, text and music from your tape when it starts.

## Status

Nothing to play yet. The original now boots from its tape in the 128K reference machine, through the real ROM and its own loader, to the state the checks will start from. A census of 20,000 frames of play found it running no ROM code, and reading the ROM in three places, one of which feeds its random numbers (#21). The game can read its tape: it refuses anything but the one supported dump, and builds the 128K's memory banks from the tape alone, with no ROM. A second census found the game reading only one byte the tape didn't load, which is zero both in the original and in what the game builds (`docs/re/robin.md`). The first subsystem is rewritten and checked against the original: the screen and the play area's buffers, the tables built at start-up, and the text printer (#24).

What exists is the groundwork copied from [starquake-recompiled](https://github.com/zx-recompiled/starquake-recompiled), where the same approach produced a complete rewrite of Starquake (`REUSED.md` lists what came from there):

- `crates/zx-runtime`: a reference Z80 interpreter, which runs the original for comparison, as a 48K Spectrum or a 128K one: two ROMs and eight memory banks paged through port `0x7FFD`, the shadow screen, contention by bank, and the AY sound chip's registers. The 128K's timing is the grey +2's; its frame and interrupt lengths were measured on real machines, and the rest is from the written references until hardware-checked timing tests confirm it (#12).
- `crates/zx-recomp`: traces the original as it runs and disassembles it into listings to read.
- `crates/zx-core`: the Z80 decoder, `.tap` loading, screen layout, PNG and SHA-1.
- `games/robin`: the game. So far it reads the player's tape (`robin::assets`) and draws the screen and prints text (`robin::Game`, `screen`, `print`).
- `tools/robin-verify`: the differential checks, each rewritten routine against the original's own calls.

The work ahead, in order, is on the project board: establish the facts about the tape, build the 128K reference machine, load and boot the original in it, trace it, read its assets from the tape, rewrite it subsystem by subsystem with a differential check for each, then the window, sound and input, and long runs comparing every frame.

## How it is checked

Each rewritten routine is run beside the original's, which runs in the reference interpreter (`tools/robin-verify`).
- **Real calls.** The original boots from the tape and plays 20,000 frames of scripted and random input. Every call it makes to a rewritten routine is caught at the routine's entry. The original routine runs alone to its return, and the rewrite runs from the same state. All of memory is compared, with the routine's outputs and any port it writes. Calls whose registers and every byte read were seen before are counted and skipped.
- **Each distinct call runs twice more.** Once with the bytes it only writes changed first, so a write the rewrite leaves out shows even where the old value happened to be right. Once with the screen and play-area data it read changed, for values play never shows.
- **Registers.** After a return, every register and flag that isn't a declared output is scrambled. The original plays on, interrupts and input included, until the caller's stack is back where it was, and must go the same way and write the same memory as without the scrambling. That shows no caller reads them.
- **The report** lists every instruction of a routine that no call reached: code play never ran, checked only by reading.

Each part of this was shown to fail on a planted bug, and the verifier's own tests check it on a made-up program in CI.

Long runs will compare every frame of random play too. Where the rewrite cannot match the original exactly, this README will say so and why.

Those checks can only be as good as the interpreter, so it is checked against outside references rather than against this project's own work:

- **[z80test](https://github.com/raxoft/z80test)**, by Patrik Rak, whose expected results were measured on a real Spectrum, is the authority on flags and registers. All three of its programs that are run pass. It confirms only part of MEMPTR, one of the processor's hidden registers; see starquake-recompiled#145.
- **The [Fuse](https://fuse-emulator.sourceforge.net/) project's Z80 test corpus** is kept for timing. Six of its 1335 cases expect behaviour z80test shows to be wrong, and they are listed with the evidence.

```sh
cargo test -p zx-runtime --test z80test --test fuse -- --nocapture
```

Their files are fetched, never committed; `assets/README.md` says where from.

The machines around the processor are checked the same way, against Spectrum test programs whose results were measured on real machines. Each machine boots from its real ROM, loads the program from its tape through the ROM's own `LOAD`, and the verdict is read off the screen (`crates/zx-runtime/tests/hardware.rs`):

- Patrik Rak's `minfo` reads the frame and interrupt lengths Brendan Alford measured: 70,908 and 35 T-states on the 128K, 69,888 and 32 on the 48K. It found two faults in how the interpreter took interrupts, both since fixed.
- Rak's Timing Test prints nine tables of instruction timings. Five match photographs of a real grey +2 exactly. The other four differ in the last one to three timings of a contended line (#14).
- Philip Kendall's Fuse Test passes everything that applies, except the floating bus, which the machine does not model.
- Richard and Tim Butler's 128K tests disagree here, and in another emulator too, for a reason not yet known (#16). They are reported, not judged.

## Building

```sh
cargo build --workspace
.claude/scripts/check.sh   # the whole gate, as CI runs it and more
```

## The legal model

The repository holds only code written for this project. The game's data is read from the player's own tape, which the program will check against one known dump and otherwise refuse. The Spectrum ROMs are needed only by the development tools, to run the original for comparison; the game itself never uses them. Neither the tape nor the ROMs are ever committed, and CI fails if one is. `assets/README.md` has the details.

## Licence

The code is licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option. This covers the code only, not the game.

The code is written by Claude (Anthropic's AI model), working with @starquake, who supplies the game, decides the direction, and reviews and merges everything.
