# Robin of the Wood: reverse-engineering notes

What is known about the original program, in our own words: descriptions,
addresses and layouts, never listings of its code.

Each fact is marked with how it is known:

- **confirmed**: seen running the original in this project's reference
  machine;
- **provisional**: seen only in an emulator whose accuracy is unchecked
  (zx84), to be re-confirmed in the reference machine when the original is
  booted there (#3);
- **read**: understood from the bytes or the disassembly;
- **guess**.

The facts were first read from the tape's bytes and watched in zx84 (#1).
Booting the original in the reference machine (#3) then confirmed what is
marked **confirmed**: `games/robin/tests/boot.rs` checks the loading and the
hand-over, and `games/robin/tests/census.rs` counts the game's use of the ROM
over 20,000 frames of play. What is still **provisional** has not been seen in
the reference machine yet.

## Making a listing

The notes are written from listings: the original traced as it plays and
disassembled (#4). `tools/re/robin.toml` boots it from its tape to the
hand-over, plays 20,000 frames (0 to start, then random held keys), and
writes one file with a section per page:

```
cargo run --release -p zx-recomp -- tools/re/robin.toml --listing target/re/robin.lst
```

It needs the tape and `128.rom` in `assets/`. **A listing is the original's
code and is never committed**: keep it under `target/` (`*.lst` is ignored
everywhere, and CI refuses one). What it teaches goes here, in our own words.

Code is named by where it is, not only by the address it runs at, because
banks 0, 4 and 6 hold different code at the same addresses. Code in the bank
paged at `0xC000` is `bank:address` (`0:CD5F`); the ROM and banks 5 and 2,
which never move, are the plain address (`5B8A`). The trace follows the code
it saw running in each bank, and the banked-call routine at `0x5B8A`
(*Memory and paging*) is decoded so the code it calls is found even when the
trace never reached it.

## The tape

- **The dump**: a TZX 1.10 file, SHA-1
  `2aad3402cdc08907000900c5da8c29eeb2f48c9e`, 62,599 bytes. **read**
- **Every data block is at the ROM's standard speed** (TZX block `0x10`). There
  are no turbo, pure-data or custom blocks, so a reader needs only standard
  blocks, plus skipping the archive-info block (`0x32`) at the start.
  **confirmed** (`zx_core::tape::load_tzx` reads it)
- The blocks, in order: **confirmed**

  | # | What | Length |
  |---|---|---|
  | 1, 2 | BASIC program `rotw`, autostarting at line 0 | 60 bytes |
  | 3, 4 | code `r1`, loading at `0x5000` | 256 bytes |
  | 5 | headerless: the loading screen | 6,912 bytes |
  | 6 | headerless: the main program | 34,560 bytes |
  | 7 | headerless | 4,096 bytes |
  | 8 | headerless | 16,384 bytes |

- The archive-info block, written by whoever made the dump ("TZXed by Andrew
  Barker"), titles it *Robin Of The Wood (128k Version)*, by Odin Computer
  Graphics, 1986, and credits Steve Wetheril, Paul Salmon and Fred Gray. It
  calls the dump a backup of the original release. That is the dump maker's
  record, not the inlay's. **read**

## The loader

- The BASIC program clears the screen to black, loads `r1` with
  `LOAD "" CODE`, and calls it at `0x5000`. **read**
- `r1` is the game's own loader, though the loading itself is the ROM's. It
  sets its stack below itself (`0x508C`). Then, for each headerless block,
  it pages memory through `0x7FFD`, sets the load address and length, and
  enters the ROM's tape loader. Only the first 76 bytes of `r1` are code; the
  rest are never reached. **read**, and the hand-over **confirmed**
- **It enters the ROM loader at `0x0563`, not at `LD-BYTES` (`0x0556`).** It
  does `LD-BYTES`'s first few instructions itself and jumps past the rest of
  its preamble. `0x0563` is the operand byte of an `IN A,(0xFE)`, so the ROM
  resumes on a different instruction from the one its authors wrote. This
  skips setting the border, reading the EAR level, and pushing the address
  of the routine that finishes a load. So `LD-BYTES` returns straight to
  `r1`, with no BREAK check and interrupts left disabled. A loader trap
  placed on `LD-BYTES` would never fire; it has to be at `0x0563`. **read**
- Where each block lands, with the value written to `0x7FFD` first:
  **confirmed**, every byte (the facts are `games/robin/src/layout.rs`)

  | Block | `0x7FFD` | Lands at |
  |---|---|---|
  | loading screen | `0x1F` | bank 7, `0xC000`, and bank 7 is also the screen shown: the shadow screen |
  | main program | `0x18` | `0x5B00`–`0xE1FF`: the rest of bank 5, all of bank 2, and bank 0's first `0x2200` bytes (`0xC000`–`0xE1FF`) |
  | 4,096 bytes | `0x1E` | bank 6, `0xC000`–`0xCFFF` |
  | 16,384 bytes | `0x1C` | all of bank 4 |

  Every value has bit 4 set: the ROM paged in is ROM 1, the 128's own copy of
  48 BASIC. **That ROM is not `48.rom`**: they differ in 1,177 bytes, so the
  reference machine has to use the second half of `128.rom`.
- At the end, `r1` writes `0x10` to `0x7FFD` (bank 0 at `0xC000`, the normal
  screen in bank 5, ROM 1, paging not locked) and jumps to `0x5B00`.
  Interrupts are disabled, the processor is in IM 1, and SP is `0x508C`.
  **confirmed**

## Where the program starts

- `0x5B00` writes `0x10` to `0x7FFD` again and jumps to `0xBE4A`, the
  program's own start. That disables interrupts, sets its stack to `0x5B8A`,
  initialises, and enters its main loop. **read**; the jump to `0xBE4A`
  **confirmed**
- **Entry state for the checks**: PC `0x5B00`, SP `0x508C`, interrupts
  disabled, IM 1, `0x7FFD` = `0x10`, with banks 0, 2, 4, 5, 6 and 7 loaded as
  above. Banks 1 and 3 are never loaded. **confirmed**

## Memory and paging

- **The game pages memory while it runs.** It calls code in other banks
  through a routine at `0x5B8A`. That routine reads a 16-bit address and a
  bank number from the bytes after the call, remembers the bank paged now,
  pages the target bank in at `0xC000` with ROM 1 selected, calls the
  address, and restores the previous bank on the way back. It disables
  interrupts on entry. **read**
- Values written to `0x7FFD` while it runs: `0x10`, `0x14` and `0x16`, that is
  banks 0, 4 and 6, always with ROM 1 and the normal screen. Bank 4 was seen
  paged only at the menu, bank 6 at the menu and in play. **provisional**
- **The screen never moves to bank 7 after loading, and paging is never
  locked**: bits 3 and 5 of `0x7FFD` stay clear. **provisional**

## Interrupts

- **IM 2, through the classic all-`0xFF` table.** I is `0xE2`, and
  `0xE200`–`0xE300` (257 bytes, in bank 0, just past what the tape loads, so the game builds it) are all `0xFF`, so the vector is
  `0xFFFF` whatever the bus carries. At `0xFFFF` is a relative jump whose
  displacement is the next byte, which wraps round to the ROM's first byte
  (`0xF3`), landing at `0xFFF4`. That jumps to the handler at `0xDED3`.
  **read**; the jump at `0xFFFF` reading the ROM's first byte **confirmed** by
  the census (every frame of play), the handler's address **provisional**.
  That the game writes the table before the first interrupt is **confirmed**:
  every vector read in 20,000 frames found bytes it had written
  (`games/robin/tests/uninitialised.rs`).
- So **the game depends on the ROM's first byte as data**, though it never
  runs the ROM's interrupt routine. **read**

## Sound

- **It uses the AY sound chip.** It selects registers through `0xFFFD` and
  writes them through `0xBFFD`, every frame in play: registers 0 to 10 and 13
  (tone periods, noise, mixer, volumes, envelope shape). It also reads
  registers back through `0xFFFD`. The code is at `0xC0xx`–`0xC2xx`, in a
  paged bank. **provisional**
- **It uses the beeper too, at the menu**: port `0xFE` takes `0x00`, `0x08`,
  `0x10` and `0x18` from one place (`0xC0AE`), toggling both EAR (bit 4) and
  MIC (bit 3). None was seen in a short stretch of play. **provisional**
- The menu offers *ENTER = music on/off*. **provisional**

## Input

- The menu offers keyboard, Kempston and Interface II. **provisional**
- In play, with the keyboard, it reads the half-rows `0xF7FE` (1–5), `0xFBFE`
  (Q–T), `0xFDFE` (A–G), `0xFEFE` (Shift–V), `0xBFFE` (Enter–H) and `0x7FFE`
  (Space–B). It does not read the row with O and P. **provisional**

## What it uses from the ROM

- **No ROM code runs.** Over 20,000 frames from the hand-over, through the
  menu and into play under random held keys, not one instruction was executed
  below `0x4000`. **confirmed** (`games/robin/tests/census.rs`)
- **It reads the ROM in three places**, and nowhere else in that run.
  **confirmed**
  - **The interrupt**: the jump at `0xFFFF` takes its displacement from the
    ROM's first byte (*Interrupts*).
  - **A delay**: a routine at `0x8B32` plays a beeper sound eleven times, each
    a little slower. Between the notes, an `LDIR` copies the 16K at `0x0000`
    onto itself. Writes to ROM go nowhere, and the flags it leaves are thrown
    away straight after, so only the time it takes matters: 16,384 uncontended
    transfers each time. Nothing depends on what the ROM holds. **read**
    (called from at least `0xB796` and `0xCD4E`; what for is #4's to find)
  - **The random numbers**: the routine at `0xCD5F` takes R, reads the byte
    at `R × 0x101`, and mixes it with R into the seed at `0xD26A`. When R is
    below `0x40` that address is in the ROM, so **the ROM's contents feed the
    random numbers** (25 reads at 4 addresses in the run). The rewrite has no
    ROM, so this needs a decision: #21. **read**
- IY is not BASIC's `0x5C3A` in play (`0xFF20` was seen), which fits a game
  that calls nothing in the ROM that needs the system variables.
  **provisional**

## What it reads that the tape did not load

The game reads its tape through `robin::assets::read_tape`. That checks the
SHA-1, then builds the eight banks from the four game blocks and the facts in
`games/robin/src/layout.rs`, with no ROM; everything the blocks don't cover is
zero. The original starts from more than that: whatever the ROM, BASIC and
`r1` left in memory, and banks 1 and 3 as they powered on. So any read of a
byte the tape didn't load, before the game writes it, is a place where the
rewrite could start differently.

- **There is one such read, and it does no harm.** Over 20,000 frames from the
  hand-over, every read of RAM was checked against a map of the bytes loaded
  or written so far, opcode fetches and the interrupt's vector reads included.
  **confirmed** (`games/robin/tests/uninitialised.rs`)
  - The routine at `0:CE27` copies the non-zero bytes of a `0x240`-byte
    buffer at `0xE800` to `0xE500`. Its loop ends only when the count goes
    below zero, so it handles one byte more than it was asked to and reads
    `0:EA40`, just past the buffer. Nothing loads that byte and nothing
    writes it. It is zero in the booted original and in the reader's banks,
    so nothing is copied; a non-zero byte would land at `0xE740`. Three
    reads in the run. **read**, the read itself **confirmed**
  - The census checks that the original and the reader agree on the byte,
    not just that the read is allowed.
- **On a real machine that byte might not be zero.** The reference machine's
  RAM powers on as zeros. A real Spectrum's may not, unless the ROM clears it
  first. **guess**

## Credits and date

- The program's own menu says "© 1986 Odin Computer Graphics". **provisional**
- The individual credits (Steve Wetheril, Paul Salmon, Fred Gray) come only
  from the dump's archive-info block, not from the inlay or the program. The
  maintainer accepted that as the README's source (#1).
