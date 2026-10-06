# Robin of the Wood, recompiled

Robin of the Wood, the ZX Spectrum 128K release (Odin Computer Graphics, 1986), rewritten from scratch in Rust to run natively: no emulator and no Z80 at runtime.

**This project is not affiliated with or endorsed by the rights holders of Robin of the Wood. You need your own copy of the game:** the program will contain no part of it, and will read the graphics, maps, text and music from your tape when it starts.

## Status

Set up, and nothing more yet. There is nothing to play.

What exists is the groundwork copied from [starquake-recompiled](https://github.com/zx-recompiled/starquake-recompiled), where the same approach produced a complete rewrite of Starquake (`REUSED.md` lists what came from there):

- `crates/zx-runtime`: a reference Z80 interpreter for a 48K Spectrum, which runs the original for comparison. The 128K machine (memory banks, paging, its timing, the AY sound chip's registers) is still to come.
- `crates/zx-recomp`: traces the original as it runs and disassembles it into listings to read.
- `crates/zx-core`: the Z80 decoder, `.tap` loading, screen layout, PNG and SHA-1.
- `games/robin`: the game, a stub so far.

The work ahead, in order, is on the project board: establish the facts about the tape, build the 128K reference machine, load and boot the original in it, trace it, read its assets from the tape, rewrite it subsystem by subsystem with a differential check for each, then the window, sound and input, and long runs comparing every frame.

## How it will be checked

Each rewritten routine will be run beside the original's, which runs in the reference interpreter, from the same starting states taken from the original's own play, and the screen and game state compared byte for byte. Long runs will compare every frame of random play the same way. Where the rewrite cannot match the original exactly, this README will say so and why.

Those checks can only be as good as the interpreter, so it is checked against outside references rather than against this project's own work:

- **[z80test](https://github.com/raxoft/z80test)**, by Patrik Rak, whose expected results were measured on a real Spectrum, is the authority on flags and registers. All three of its programs that are run pass. It confirms only part of MEMPTR, one of the processor's hidden registers; see starquake-recompiled#145.
- **The [Fuse](https://fuse-emulator.sourceforge.net/) project's Z80 test corpus** is kept for timing. Six of its 1335 cases expect behaviour z80test shows to be wrong, and they are listed with the evidence.

```sh
cargo test -p zx-runtime --test z80test --test fuse -- --nocapture
```

Their files are fetched, never committed; `assets/README.md` says where from.

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
