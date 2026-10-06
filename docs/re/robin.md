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

## The screen

Found by recording which code read and wrote which memory over 3,000 frames
of play (#24), then reading those routines. Every routine here is in bank 0,
at `0xC000`–`0xFFFF`. **read**, unless marked.

### The play area and its buffers

The play area is 18 character rows by 28 columns: screen rows 0 to 17,
columns 2 to 29. The game doesn't draw it on the screen directly. It keeps
three buffers in bank 0, past what the tape loads, and copies from them:

| Buffer | Where | Layout |
|---|---|---|
| **Back buffer**: the play area's pixels | `0xEB00`–`0xFCFF` | One 256-byte block per character row. Within it, one 32-byte line per pixel row, and column *c* at byte *c*. Columns 0, 1, 30 and 31 are unused. |
| **Attribute buffer** | `0xE800`–`0xEA3F` | 32 bytes per character row, laid out as the screen's attributes. Bit 7 is the game's own flag: it is set on cells the text printer coloured, and masked off on the way to the screen. |
| **Changed-cell map** | `0xE500`–`0xE73F` | 32 bytes per character row. Non-zero means the cell needs redrawing, and the value itself is the attribute to use if the attribute buffer holds zero there. |

Drawing into the play area means writing the back buffer and the attribute
buffer, then marking cells in the changed-cell map. Once a frame, the main
loop flushes the changed cells to the screen. **confirmed** (the flush ran in
2,481 of 3,000 frames)

### Tables built at start-up

Both are built by the start-up code that runs before the menu (`0:CC66`),
from nothing but their own arithmetic.

- **`0xFD00`, the mirror table** (`0:CEC8`): 256 bytes. Entry *n* is *n*
  with its eight bits in reverse order, so looking a byte up flips it left
  to right. The text printer uses it to print mirrored, and the sprite code
  reads it too.
- **`0xFE00`, the row table** (`0:CEDE`): the screen address of each of the
  192 pixel rows, top to bottom, as 192 little-endian words. That is the
  Spectrum's usual interleaving: the next pixel row is `0x100` on, the next
  character row `0x20` on, and the next third of the screen `0x800` on.

### Clearing

| Routine | Clears |
|---|---|
| `0:CEFE` | The screen's pixels, all 6,144 bytes, to 0. |
| `0:CF0C` | The screen's attributes, all 768, to the value in A. |
| `0:CF19` | The attribute buffer, all `0x240` bytes, to the value in A, then the back buffer, all `0x1200` bytes, to 0. |
| `0:CF33` | The changed-cell map, all `0x240` bytes, to 0. |
| `0:CF41` | The bottom 32 pixel rows of the screen (rows 160 to 191), 32 bytes each, to 0, found through the row table. |

### Copying the play area to the screen

- **The flush**, `0:C754`, once a frame from the main loop. It scans the
  changed cells of the play area row by row, left to right. For each
  changed cell:
  1. it clears the mark;
  2. it writes the cell's attribute to the screen, with bit 7 masked off. The
     attribute comes from the attribute buffer, or from the mark itself if
     the buffer holds zero;
  3. it copies the cell's 8 pixel rows from the back buffer.

  The test for zero goes through the alternate AF, so the routine leaves the
  alternate AF changed.
- **The whole play area's pixels**, `0:C6FE`: all 144 pixel rows of the back
  buffer, 28 bytes each, to the screen through the row table.
- **The whole play area's attributes**, `0:C086`: the attribute buffer's 18
  rows of 28 to the screen, bit 7 masked off.

### The text printer

One routine with three ways in. It prints a string of characters, then
colours them with a second string of attributes that follows the first.

- **`0:D4F6`**: the string is in memory just after two bytes giving its
  position. Its attributes go straight to the screen.
- **`0:D4FC`**: the same, with the position in a register pair, not in
  memory.
- **`0:D50E`**: one of 17 stock messages, chosen by the low five bits of A
  from a table of addresses at `0xD6AB`. Its attributes go to the attribute
  buffer, two columns further right (the play area's offset), and get bit
  7 set, except for message 14.

How it prints:

- **A mode byte**, passed in A and kept at `0xD6CE`, chooses where. Bit 5
  set means straight to the screen, through the row table. Clear means into
  the back buffer. Bits 6 and 7 choose what happens after the characters.
  Some combinations never occurred in 20,000 frames of play. They are noted
  as such and checked only from reading.
- **The position** is a pixel row and a horizontal position in units of two
  pixels. Characters are placed on whole columns, and attributes on whole
  character rows.
- **The font** is the tape's, 8 bytes a character, at `0xAAEF` plus 8 times
  the character code. So the space, `0x20`, is at `0xABEF`, in what the main
  block loads.
- **Codes in the string**:
  - 0 ends the characters;
  - 1 starts a new line, going back to where the line began;
  - 2 turns mirroring on, and 3 turns it off (each glyph row flipped through the mirror table);
  - codes 4 to `0x1F` are skipped;
  - `0x20` and up are printed.
- **Each glyph row is either combined with what's there (XOR) or replaces
  it.** XOR is normal. A flag at `0xD6CF` asks for replacing, and the printer
  clears that flag when it's done: one entry point, `0:CEBB`, sets it for a
  single string.
- **The attribute string**: one byte a character, written to the screen or
  the attribute buffer. A byte with bit 7 set starts a new line. 0 ends it.
- **Afterwards**, depending on the mode, it can:
  - mark the coloured cells changed, with the attribute as the mark;
  - record the message, its mode and position in a list of nine at `0xD457`,
    which `0:D6D0` checks every frame against what I take to be Robin's
    position;
  - continue into `0:DD49`.

  Neither of the first two happened in the run.
- **Its settings live in its own code.** Three of its instructions have
  their operands rewritten on each entry: the column offset, the bit-7 flag
  for attributes, and which page the attributes go to (`0x58` for the
  screen, `0xE8` for the buffer). The mirror table lookup is rewritten for
  each glyph byte.

### Not covered here

Drawn into the back buffer every frame, and their own subsystem:

- the sprite code, `0:C5CE` and `0:C47D`;
- the code that marks the cells it drew, `0:C688`.

The scenery renderer, `0:CD73` with `0:CE27`, draws each screen of the map,
and goes with the map. The bank 4 code at `4:C086` fills all three buffers
at the menu.

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
- **On a real machine that byte is zero too.** A real 128K's RAM doesn't power
  on as zeros, but nothing of what it held survives the ROM's start-up. A
  machine with every bank filled with `0xAA` at power-on reaches the menu in
  exactly the state of one that powered on as zeros, every byte and register.
  So whatever the RAM held at power-on, the original reads zero there.
  **confirmed** (`nothing_from_power_on_survives_the_roms_start_up`)

## Credits and date

- The program's own menu says "© 1986 Odin Computer Graphics". **provisional**
- The individual credits (Steve Wetheril, Paul Salmon, Fred Gray) come only
  from the dump's archive-info block, not from the inlay or the program. The
  maintainer accepted that as the README's source (#1).
