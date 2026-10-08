# Robin of the Wood, recompiled

Robin of the Wood, the ZX Spectrum 128K release (Odin Computer Graphics, 1986), rewritten from scratch in Rust to run natively: no emulator and no Z80 at runtime.

The original is by Steve Wetheril, Paul Salmon and Fred Gray, as credited in the dump's archive information.

**This project is not affiliated with or endorsed by the rights holders of Robin of the Wood. You need your own copy of the game:** the program will contain no part of it, and will read the graphics, maps, text and music from your tape when it starts.

## Status

Nothing to play yet. The original now boots from its tape in the 128K reference machine, through the real ROM and its own loader, to the state the checks will start from. A census of 20,000 frames of play found it running no ROM code, and reading the ROM in three places, one of which feeds its random numbers (#21). The game can read its tape: it refuses anything but the one supported dump, and builds the 128K's memory banks from the tape alone, with no ROM. A second census found the game reading only one byte the tape didn't load, which is zero both in the original and in what the game builds (`docs/re/robin.md`). The first subsystems are rewritten and checked against the original: the screen and the play area's buffers, the tables built at start-up and the text printer (#24); then the map, read from the tape, and drawing each of its 320 locations (#28). A tour sends the original to every location, so all of them are checked, not only those play happens to reach. The sprites follow (#32): animated, erased and drawn as the original does. Then Robin's movement (#34): the controls by any of the three methods the menu offers, walking, walls and leaving the screen. Then the four characters who walk each row of the forest (#38): put on their floors on entering a location, and moving, turning, firing and drawn, one a frame. Then Robin's actions (#36): standing, walking, attacking with his fists, the sword and the bow, being knocked down and getting up, and the sampled sound that plays when he's hit, checked write for write at the ports. Then the wanderer (#39), a friendly character who walks from location to location and gives Robin energy when he meets it. Then the fighting (#40): arrows and shots in flight, hits both ways, his sword and fists, and his health, which the lower panel's colour shows. Then the rest of the main loop (#41): the second group of characters, at the locations from 256 up; items placed, restocked, picked up, carried and dropped; the fifth character and its companion; the trade, the doorways and the journeys; and BREAK and the game over. Some of it is rewritten from reading the code and not yet checked, because nothing yet reaches it or because it reads the ROM (#21): the trade itself, the fifth character being robbed, and the journeys (`docs/re/robin.md` says which).

**Where the rewrite will not match exactly: randomness.** The original takes its random numbers from the Z80's refresh register, R, which counts the instructions the processor has fetched, and in one routine from bytes of the ROM (#21). A rewrite in Rust runs different instructions and has no ROM, so it cannot reproduce them. The checks give the rewrite the very values the original read, so every routine is still compared exactly. But the rewritten game will draw its own random numbers, and will not repeat the original's sequence: the same play will not unfold exactly as it would on a Spectrum (#37). Where the original reads the ROM for a sprite, which it can when a certain character is met in the first 18 seconds of a game, the rewrite cannot match it; that is #21, with the random numbers.

What exists is the groundwork copied from [starquake-recompiled](https://github.com/zx-recompiled/starquake-recompiled), where the same approach produced a complete rewrite of Starquake (`REUSED.md` lists what came from there):

- `crates/zx-runtime`: a reference Z80 interpreter, which runs the original for comparison, as a 48K Spectrum or a 128K one: two ROMs and eight memory banks paged through port `0x7FFD`, the shadow screen, contention by bank, and the AY sound chip's registers. The 128K's timing is the grey +2's; its frame and interrupt lengths were measured on real machines, and the rest is from the written references until hardware-checked timing tests confirm it (#12).
- `crates/zx-recomp`: traces the original as it runs and disassembles it into listings to read.
- `crates/zx-core`: the Z80 decoder, `.tap` loading, screen layout, PNG and SHA-1.
- `games/robin`: the game. So far it reads the player's tape (`robin::assets`), draws the screen and prints text, draws the map's locations and the sprites, moves Robin and runs his actions, runs the characters on each row, the wanderer and the fifth character, the fighting, the items, the trade and the journeys, the main loop's own checks, and the sounds as port writes (`robin::Game`, `screen`, `print`, `map`, `sprites`, `movement`, `actions`, `characters`, `wanderer`, `fifth`, `fighting`, `items`, `journeys`, `main_loop`, `sound`).
- `tools/robin-verify`: the differential checks, each rewritten routine against the original's own calls.

The work ahead, in order, is on the project board: establish the facts about the tape, build the 128K reference machine, load and boot the original in it, trace it, read its assets from the tape, rewrite it subsystem by subsystem with a differential check for each, then the window, sound and input, and long runs comparing every frame.

## How it is checked

Each rewritten routine is run beside the original's, which runs in the reference interpreter (`tools/robin-verify`).
- **Real calls.** The original boots from the tape and plays 20,000 frames of scripted and random input. Every call it makes to a rewritten routine is caught at the routine's entry. The original routine runs alone to its return, and the rewrite runs from the same state. All of memory is compared, with the routine's outputs and every port it writes, value for value and in order. Calls whose registers and every byte read were seen before are counted and skipped.
- **Each distinct call runs twice more.** Once with the bytes it only writes changed first, so a write the rewrite leaves out shows even where the old value happened to be right. Once with the screen and play-area data it read changed, for values play never shows.
- **Registers.** After a return, every register and flag that isn't a declared output is scrambled. The original plays on, interrupts and input included, until the caller's stack is back where it was, and must go the same way and write the same memory as without the scrambling. That shows no caller reads them.
- **A tour of every location.** After play, the original is walked round all 320 locations of the map, and every call it makes is compared too; play alone reached about 21. Robin walks off each screen through the controls where the way is clear, and is sent on through the original's own step and entry where it isn't. The gate walks him off the screen at every move (#46).
- **Every control method.** A short game is also played with each control method the menu offers, chosen through the menu as a player would.
- **Robin armed.** One more game starts with Robin given the sword, the bow, ten arrows and some energy, as the game itself gives them later on, so his attacks with them, his arrows landing (#40), and being knocked down and getting up again, are checked too (#36). Play never gets that far.
- **Things to pick up.** A last game starts Robin beside seven items of different kinds, with his inventory full, so picking each up, carrying it, and the drop when his inventory overflows are checked too (#41). Play almost never walks him into one.
- **The report** lists every instruction of a routine that no call reached: code play and the tour never ran, checked only by reading.

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
