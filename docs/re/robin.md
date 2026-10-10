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
  the census (every frame of play), the handler's address **confirmed** (#62).
  That the game writes the table before the first interrupt is **confirmed**:
  every vector read in 20,000 frames found bytes it had written
  (`games/robin/tests/uninitialised.rs`).
- So **the game depends on the ROM's first byte as data**, though it never
  runs the ROM's interrupt routine. **read**
- **The handler** (`0:DED3`, **confirmed**): it saves every register, calls
  three parts of the music player in bank 6 through the trampoline, and
  restores them. **Rewritten (`games/robin/src/interrupt.rs`) and confirmed**
  (#62):
  1. **ENTER and the tune** (`6:C2CF`, through `6:C03C`, *Sound*);
  2. **the wobble** (`6:C26F`, through `6:C030`);
  3. **the effect** (`6:C1EB`, through `6:C015`).

## The screen

Found by recording which code read and wrote which memory over 3,000 frames
of play (#24), then reading those routines. Every routine here is in bank 0,
at `0xC000`–`0xFFFF`.

**Every routine in this section is rewritten (`games/robin/src/screen.rs`,
`print.rs`) and confirmed** against the original by `tools/robin-verify`.
Over 20,000 frames of play, each distinct call the original made was run
three ways and compared with the rewrite on all of memory:
- as it was;
- with the bytes it only writes changed first;
- with the screen and play-area data it read changed.

For the first 20 distinct calls from each caller, scrambling the
registers these routines leave, after the return, changes nothing. The
original plays on, interrupts and input included, until the caller's stack
is back where it was, and goes the same way and writes the same memory. So
no caller was seen reading them. The exceptions, paths
play never took, are marked **read** below.

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
- **The reveal**, `0:CD73`, when a new game starts and at the scripted
  scene's end (#42). **confirmed**, every instruction reached at each new
  game, and picture by picture: the checks compare the screen at the end
  of each of its passes with the original's (#57), which is the only way to
  see the slide, since the last pass overwrites every column it moved:
  1. every set colour in the attribute buffer is copied into the
     changed-cell map;
  2. in 14 passes, each half of the play area (columns 2 to 15, and 16 to
     29) slides one column out towards its edge, and the new screen's next
     column goes in beside the middle: its colour from the changed-cell
     map, flash masked off, and its pixels from the back buffer.

  Its block copies are instructions it rewrites: for the last pass, which
  copies nothing, their first byte becomes 0, leaving a `NOP` and an `OR B`.
  The next reveal makes them `LDIR` and `LDDR` again.

### The text printer

One routine with three ways in. It prints a string of characters, then
colours them with a second string of attributes that follows the first.

- **`0:D4F6`**: the string is in memory just after two bytes giving its
  position. Its attributes go straight to the screen.
- **`0:D4FC`**: the same, with the position in a register pair, not in
  memory. Play only ever reaches it by running on from `0:D4F6`: its
  direct callers (`0:D868`, `0:DAB0`, `0:DE8D`) never ran in the 20,000
  frames.
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
  clears that flag when it's done. One entry point, `0:CEBB`, sets it for a
  single string. It also overwrites A before passing it on, so it always
  prints in mode `0x60` (to the screen, not recorded), whatever A its caller
  set.
- **The attribute string**: one byte a character, written to the screen or
  the attribute buffer. A byte with bit 7 set starts a new line. 0 ends it.
- **Afterwards**, depending on the mode, it can:
  - mark the coloured cells changed, with the attribute as the mark;
  - record the message, its mode and position in a list of nine at `0xD457`,
    which `0:D6D0` checks every frame against what I take to be Robin's
    position;
  - with bit 7, write zeros for the attributes, mark the cell the last
    string ended at, and continue into `0:DD49`. That saves row 12 of the
    attribute buffer, across the play area, at `0xDDB7`.

  **None of these ran in 20,000 frames of play**, nor did message 14's
  unflagged attributes. The rewrite does them from this reading alone, and
  the check's report lists the 36 instructions of the printer no call
  reached. **read**
- **Its settings live in its own code.** Three of its instructions have
  their operands rewritten on each entry: the column offset, the bit-7 flag
  for attributes, and which page the attributes go to (`0x58` for the
  screen, `0xE8` for the buffer). The mirror table lookup is rewritten for
  each glyph byte.

### Not covered here

Drawn into the back buffer every frame, and their own subsystem: the
sprites (*Sprites*).

The scenery is drawn by `0xBF6A` (*The map*). `0:CD73` with `0:CE27`, first
taken for the scenery renderer, is a transition effect that goes with the
menus. The bank 4 code at `4:C086` fills all three buffers
at the menu.

## The map

Found by watching which code draws the play area's scenery during play, then
reading it (#28).

**Everything in this section is rewritten (`games/robin/src/map.rs`) and
confirmed** against the original by `tools/robin-verify`, from play and from
a tour of every location. The tour walks the original round all 320
locations, each reached through the original's own step and entry. Every
call it makes to the drawing is compared, as in play. Location `0x69` is
drawn by its drawing routine alone (*The grid*). Every instruction of the
drawing and the step was reached.

### The grid

- **The forest is 16 locations wide and 20 high, and wraps at every edge.**
  The current location is the word at `0xC440`, row × 16 + column, so 0 to
  `0x13F`. **confirmed** (the step below)
- **The step**, `0:C127`, moves it by the bits of A: bit 0 right, bit 1
  left, bit 2 down, bit 3 up. Each wraps: column 15 to 0, row 19 to 0, and
  back. The game takes it when Robin leaves the screen by an edge
  (`0xBECE`), then enters the new location.
- **Entering a location**, `0xBF0E`, does these in turn:
  1. clears the play area;
  2. draws the scenery (`0xBF6A`, below) and the three special locations'
     extras (`0:C056`);
  3. sets up what else is there, the characters and objects (`0:C33C`,
     `0:C306`, `0:C35A`, `0:C16E`, `0:D484`: other subsystems);
  4. copies the whole play area to the screen.
- **Location `0x69`**, row 6, column 9, is not entered that way. Walking into
  it takes another path, with the border flashing and a wait for a key. It
  looks like part of the ending.
- In 20,000 frames of play, Robin never left rows 16 to 19, about 21
  locations in all. So the extras for locations below 256 never ran in
  play, and only the tour reaches them. **confirmed**

### The tables

All are in the main block, read straight from the tape.

| Table | Where | What |
|---|---|---|
| **Locations** | `0x7AC2`, 320 bytes | One per location: the layout number (bits 0–6), and bit 7 to draw it mirrored left to right. 116 of the 320 are mirrored, so one layout serves several locations. |
| **Layouts** | `0x7C02`–`0x8553` | 128 records, one after another. Each is a count, then that many items of two bytes: a position (the character row in bits 3–7, and a column in steps of 4 characters in bits 0–2), then a block number (bits 0–6), with bit 7 to mirror that block. Layout *n* is found by stepping over the *n* before it (`0:C0B9`). |
| **Extras** | `0x85DA`–`0x8AFF` | 256 more records of the same kind, one for each location below 256: blocks drawn on top of the layout. 134 of them are not empty. |
| **Blocks** | addresses at `0x5BC9`, 84 blocks | See below. |

### A block

- **A header byte.** Its height in character rows is in bits 0–2. Bit 6 means
  one attribute colours the whole block. Bit 7 is the way it currently
  faces: 0 as on the tape, 1 mirrored. Bits 3–5 are clear in all 84.
- **Then the pixels**: 8 pixel rows per character row, 4 bytes each, so 32
  pixels wide.
- **Then the attributes**: 4 a character row, or the single one.
- **The game mirrors a block in place, in memory.** When a block is wanted
  the other way round from how it faces now (`0:C0CE`), it flips the header's
  bit 7. Then it swaps each pixel row's 4 bytes end for end, each through the
  mirror table, and swaps each attribute row's 4 bytes end for end. So the
  blocks' bytes are part of the game's state, not constant data.

### Drawing a location

`0xBF6A`, into the play area's buffers (*The screen*):

1. It looks the location up in the location table. It keeps the layout
   number at `0xC43F` and the table's address at `0xC442`. If the location is
   mirrored, it patches the drawing code, so every column is reflected
   (`column × 4 XOR 0x1C`).
2. It draws the layout's items (`0xBFA7`, `0xBFAA`). For each one, it draws the
   block (`0xBFC9`), first mirroring it in place if the way it faces differs
   from the way it's wanted (the item's bit 7, XOR the location's).
3. **Drawing a block** writes its pixel rows into the back buffer, at
   `0xEB00` + row × 256 + column × 4, then its attributes into the attribute
   buffer. Its code starts in bank 2 and runs on into bank 0 at `0:C002`,
   since the main block loads both contiguously.
4. For a location below 256, it then draws that location's extras the same
   way, with `0xC43F` and `0xC442` pointed at the extras table.

`0:C056` adds 4 more blocks when the location is one of three kept as
words at `0xD28F`, `0xD291` and `0xD293`. Those are set when a game starts
(`0:CE57`–`0:CE88`), so they move from game to game. Their lists are at
`0xC462`, `0xC46B` and `0xC474`.

### Not covered here

`0:CD73`, called in #24 the scenery renderer, isn't one. It redraws the
screen from the screen itself and the row table, as a transition: it ran
3 times in 20,000 frames, at the menu and twice in play. It goes with the
menus.

## Sprites

Found by reading the code that draws into the back buffer every frame
(#32).

**The engine is rewritten (`games/robin/src/sprites.rs`) and confirmed**
against the original by `tools/robin-verify`, in play and on the tour,
which now lets each location's characters move for half a second. The
exceptions are the calls in which the original reads the ROM (*When a
sprite reads the ROM*, below). Those are counted, not compared.

### A sprite

- **A sprite is a record** (in `IX`; Robin's is at `0xCB76`). It holds:
  - an animation counter, and the value it's reset to;
  - a pointer into an animation sequence;
  - flags: active, drawn, and to be drawn;
  - the frame and position it was last drawn at;
  - the frame and position to draw it at next.

  The records belong to the characters, whose subsystems come later.
- **An animation sequence** is a list of frame numbers. `0xFF`, followed by an
  address, goes on from there instead, so a sequence can loop.
- **Animating and redrawing a record**, `0:C7EF`, called from 9 places, one
  per kind of character:
  1. If the record is active, it counts the animation down. When the counter
     runs out, it resets it and takes the next frame number from the
     sequence, following a loop where there is one.
  2. If the sprite is drawn, it draws the old frame at the old position
     again. Drawing is XOR, so that erases it.
  3. If the sprite is to be drawn, it copies the new frame and position over
     the old ones, marks the sprite drawn, and draws it.

### Frames

- **Three frame tables**, each a list of addresses: `0x8C25` (Robin's, 34
  frames), `0x8C69` (19) and `0x8C8F` (20). Which one a sprite uses is
  patched into the drawing code by its caller (`0:C5CF`).
- **A frame starts with a header byte.** Bit 7 is the way it faces now: the
  frames are mirrored in place, like the map's blocks. 15 of the tape's
  frames start out mirrored. Bits 0–3 are its kind: 2 or 3 for a frame
  drawn as pixels, and 6 for a figure drawn from character cells.
- **A pixel frame** follows its header with its height in pixel rows (32 or
  16 on the tape), a colour byte, then 3 bytes, 24 pixels, a row.
- **A character figure** follows its header with 30 character codes (6
  wide, 5 high; 0 for none), then their 30 attributes. Its characters are
  8 bytes each, at `0x9958` + 8 × code.
- **The colour byte** picks an attribute pattern from a table of addresses
  at `0x8CB7`. Bits 7 and 6 say whether the pattern steps along each row of
  cells and down each column, or repeats one attribute.

### Drawing a frame

`0:C5CE` draws frame A at pixel position C, B, the horizontal position in
2-pixel units, into the back buffer:

1. **A character figure** (`0:C47D`) is drawn whole and returns. Each cell
   is XORed into the back buffer, mirrored if the figure faces the other way:
   the columns in reverse order (a table at `0xC57A`), and each byte through
   the mirror table. Each of its non-zero attributes then goes into the
   changed-cell map, where that cell isn't already marked.
2. **For a pixel frame**, it first marks the cells the frame covers changed
   (`0:C688`), with the attributes of its colour pattern. That is 4 cells
   wide, and as many rows as the height covers, one more if the position
   isn't on a character row.
3. If the frame faces the other way from how it is wanted, it is mirrored in
   place (`0:C7AF`). That flips the header's bit 7, then for each row swaps
   the outer two bytes and mirrors all three through the mirror table.
4. It draws each row: the 3 bytes are shifted right by the position's low
   bits, 2 pixels a step, into 4 bytes, and XORed into the back buffer.
   - The shift is an unrolled chain of shift instructions (`0:C645`). The
     routine cuts it at the right length by writing a `RET` into it, and
     puts the byte back afterwards.
   - The routine's other choices are patched into its own code too: which
     frame table, the mirroring path, and the colour pattern's steps.

Quirks the rewrite copies:
- A pixel frame at a horizontal position below 8 is drawn one pixel row
  higher. The subtraction that finds its column borrows, and the borrow is
  taken off the row.
- A character figure's drawing doesn't return to `0:C5CE`. It drops
  `0:C5CE`'s return address and returns straight to its caller.
- The shift chain's last stop starts out as a `RET`, and stays `0xCB` once
  any sprite has been shifted by 6 pixels.
- An animation counter is reset when its top bit is set after counting
  down, not only at zero.

### When a sprite reads the ROM

**confirmed**, on the tour.
- A character tied to one of the three special locations (`0xD291`, *The
  map*) has its record at `0xBB1B`. On the tape, its animation pointer is
  `0x0000`.
- That pointer is set only by a timer in `0xBA83`, which first fires about
  18 seconds into a game.
- Until then, the original takes the character's animation from the ROM:
  ROM bytes become its frame numbers, and so its frame data.

So if Robin reaches that location early enough, the original draws it from
the ROM. Play never got there, but the tour did. The rewrite has no ROM,
so it can't match those calls. `robin-verify` counts them as skipped, with
the first such read named. What the rewrite should do there is #21's
question, along with the random numbers.

## Robin's movement

Found by reading Robin's update and the code it calls (#34).

**The controls, walking, the wall tests and the edge are rewritten
(`games/robin/src/movement.rs`) and confirmed** against the original by
`tools/robin-verify`. That covers play, the tour (which now walks Robin off
the screen), and a short game with each control method chosen through the
menu. Robin's update (`0:C59A`), which runs his actions, is rewritten and
confirmed with them (*Robin's actions*, #36). **read**, unless marked.

### Robin

- **Robin is a sprite**: his record is at `0xCB76` (*Sprites*), and his
  position is its next position, `0xCB7F` horizontally and `0xCB80`
  vertically. Next to it are his direction (`0xCB85`) and a state byte
  (`0xCB81`).
- **His update**, `0:C59A`, called from the main loop (**read**):
  - it counts down a counter kept in its own code, and acts when it runs
    out;
  - it then resets the counter to 1, so it acts on every call, or to 3,
    every third, while he's fighting (`0xCB74` or `0xCB75` non-zero);
  - when it acts, it moves him (`0:C852`), runs his actions (`0:C8DF`,
    *Robin's actions*), and redraws his sprite from frame table `0x8C25`.
  - **confirmed**: rewritten (`robin::actions::update`) and checked (#36).

### The controls

`0:D0C6` reads the controls through the method chosen at the menu, kept as
an address at `0xD152`. It returns one byte, in the same bit order for
every method: bit 0 right, 1 left, 2 down, 3 up, 4 fire. Opposite
directions pressed together cancel out.

| Menu key | Method | How it reads |
|---|---|---|
| (the tape's) and 1 | Redefined keys, `0xD06E` | Five key codes at `0xD154`, in the order fire, up, down, left, right. Each code names a half-row of the keyboard (its low three bits) and a key in it (the rest). Key 1 first lets the player set them, on the "SELECT KEYS FOR" screen, which names each key from a table at `0xD159`. |
| 2 | Kempston joystick, `0xD07F` | Port `0x1F`, whose bits are already in that order. |
| 3 | Sinclair joystick, `0xD088` | The keys 6 to 0, moved into that order. |

### Walking

`0:C852`:

1. **It turns only when aligned.** It takes the controls and keeps the
   direction in `0xCB85`. A change between left and right takes effect
   only when his horizontal position is a multiple of 4. A change between
   up and down only when his vertical position is a multiple of 8. The fire
   bit only when both are.
2. **It moves**, unless his state byte is 6 or more: one step right or left,
   two down or up, each only if no wall is in the way.
3. **A wall** is a non-zero attribute without bit 7 in the play area's
   attribute buffer (*The screen*). The test looks at the cells just past
   Robin's edge in the direction he's going (`0:DC5B`, `0:DC6C`, `0:DC7F`,
   `0:DC8B`, through `0:DCC2`): two cells for a side, three for the top or
   the bottom. A step that doesn't reach a new cell boundary isn't tested.

### Leaving the screen

The main loop, after everything else (`0xBE9B`), checks Robin's position
against the play area's edges:

| Where he is | Direction | He comes in at |
|---|---|---|
| horizontal below `0x0B` | left | horizontal + `0x6E` |
| horizontal `0x7A` or more | right | horizontal − `0x6E` |
| vertical below `0x28` | up | vertical `0x68` |
| vertical `0x70` or more | down | vertical `0x30` |

Then it takes the step (`0:C127`) and enters the new location (`0xBF0E`,
*The map*). Otherwise it starts the loop again.

## Robin's actions

Found by reading `0:C8DF` and the code it reaches (#36).

**Rewritten (`games/robin/src/actions.rs`, `sound.rs`) and confirmed** against
the original by `tools/robin-verify`. That covers:
- play, the tour, and the control-method games;
- a game with Robin armed from its start: the sword, the bow, ten arrows
  and energy 9.

Between them they reach all 16 states, attacking with each weapon,
arrows fired and the last one's message, being knocked down and getting up
again both ways, and the low-energy message. **confirmed**, unless
marked.

### His state

- **His state byte, `0xCB81`, is one of 16 states**, each with a handler,
  found through a table of addresses at `0xCC3C`:

  | States | What he's doing |
  |---|---|
  | 0, 1 | standing |
  | 2, 3 | walking right, left |
  | 4, 5 | walking up, down |
  | 6, 7 | a stroke of the sword |
  | 8, 9 | firing the bow |
  | 10, 11 | a blow of his fists |
  | 12, 13 | the sword's other stroke, with down held |
  | 14, 15 | knocked down |

  Bit 0 of the state is the way he faces in every pair: 0 right, 1 left.
- **Each handler reads his controls** (`0xCB85`, *The controls*) and either
  leaves things as they are, or names his next state and the animation
  sequence his sprite is to play. Both are then stored: the state in
  `0xCB81`, the sequence in his sprite record (`0xCB78`, *Sprites*).
- **The sequences** are at `0xCB97`–`0xCC3B`: frame numbers, ending in
  `0xFF` and the address to loop back to. The last standing sequence is
  kept at `0xCB82`/`0xCB83`, so walking up or down with no direction left
  returns to it.

### Starting an attack

**Fire starts an attack** when all of these hold:
- he isn't already attacking (`0xCB74` is 0);
- he isn't knocked down (`0xCB75` is 0);
- he's aligned to the grid (horizontal position a multiple of 4, vertical a
  multiple of 8);
- no direction up or down is held.

The attack's way is the direction held, or the way he faces. With a
direction held, its sequence starts a frame earlier. Its counter, `0xCB74`,
starts at 16, and which attack it is depends on what he carries:

1. **The bow**, if he has it (`0xD47B`) and arrows are left (`0xD47D`),
   and no character on the screen is within `0x32` of him. The game sets
   the flag when it gives him the bow, with ten arrows. While it's set, the
   characters always fire (*The four on each row*). Each shot uses an
   arrow; when the last goes, a message is printed (`0xB423`).
2. **The sword**, if a flag at `0xD47A` is set: states 6 and 7, or 12 and
   13 if he's holding down (which can only be so when he attacks again
   without letting go of fire).
3. **His fists**, otherwise.

### While attacking

- **Each frame of an attack counts `0xCB74` down.** On the attack's 7th
  frame, the bow's states fire an arrow (`0xBB43`):
  - only if its slot in the objects in flight, `0xBE3F`, is free;
  - only if he isn't within two columns of either edge of the play area;
  - with a short beeper twang (`0xBE2C`);
  - the arrow's column, row and way go into the slot (#40 for its flight).
- **When the counter runs out**, he attacks again if fire is still held.
  Otherwise he walks the way held, or stands.

### Knocked down

- **When `0xCB75` is `0x6E`** (set when a hit leaves him at his lowest
  health, *Fighting*):
  - his attack counter is cleared;
  - a sound plays (*Sound*);
  - his energy, `0xD481`, drops by 2;
  - `0:D7F7`: if it's now below 9 (or `0xFF`), it prints a message
    (`0xB3DB`), adds 1 back, and prints it as a digit on the panel;
  - his controls are overridden to do nothing (`0xD0CE`, *The controls*);
  - he goes to state 14 or 15.
- **In states 14 and 15, `0xCB75` counts down.** When it reaches 0:
  - he's back in state 0;
  - the override is cleared;
  - the lower panel's colours are reset (`0xBE0F`: the 64 attribute cells
    from `0x5A40` take a colour from `0xBE47`).
- **With his energy negative, he doesn't get up.** The main loop checks
  (`0xBF3F`): once the counter reaches 60 with his energy below 0, it prints
  a message (`0xB51F`), plays a tune, waits for a key and starts a new game.
  His energy is 0 when a game starts, so in play the first knock-down ends
  it. **read** (the main loop is #41's)

## The characters

Found by reading the main loop's character routines (#37, #38). **read**,
unless marked.

### Who they are

In 20,000 frames of play, three main-loop routines drew characters:

- **four characters who walk the forest's rows**, run by `0xA8D6` (this
  section);
- **the wanderer**, the character at `0xBB1B` (`0xBA83`, *The wanderer*), confirmed;
- **Robin** (`0:C59A`, *Robin's movement*).

Others are drawn in situations play never reached (#42). Randomness: the
game reads the refresh register R in 19 places. The rewrite is given the
values the original read (#37).

### The four on each row

**confirmed**: the rewrite (`robin::characters`) matches the original in
every call play and the tour make (#38), unless marked.

- **Each row of the map has four characters.** Their list is one of four at
  `0x8B02`, 12 bytes each, chosen by the row number modulo 4 (`0:C2F2`), so
  rows 0, 4, 8 and so on share theirs. `0xC444` points at the current row's.
  Each character is three bytes:
  - the column of the map it's in;
  - its position within that screen, from 0 to `0x6F`, in units of 2
    pixels;
  - a state byte: bit 7 the way it faces (set for right), bit 6 the way it
    faced before it last turned, bit 4 set while it stands still, and bit 5
    set to make it stop (#42).
- **The floors they walk.** A screen's floor is row 12 of its attribute
  buffer, across the play area: a byte that isn't 0 is in the way. Three are
  kept: the screen to the left (`0xDD9B`), the current one (`0xDDB7`) and
  the one to the right (`0xDDD3`), 28 bytes each.
  - On entering a location sideways, the old screen's floor becomes the
    neighbour on the side Robin came from, and the other neighbour is
    cleared.
  - Entering up or down clears both neighbours.
  - Either way, the new screen's floor is copied in (`0:DD49`–`0:DD8F`).
  - A character at position `p` stands on the three bytes from
    `p / 4 − 4` of its screen's floor, four if `p` isn't a multiple of 4.
    The `− 4` reaches into the bytes before each floor.
- **Entering a location** (`0:C16E`, called from the entry, `0xBF0E`):
  1. The columns of the four characters two rows above are shuffled, each
     XORed with R and kept below 16. Their bits 4 and 5 are cleared.
  2. The current row's list is taken, and the controls' override is
     cleared.
  3. At the location kept at `0xD295`, a scripted scene starts (*The
     scripted scene*). Two
     of the four are put either side of Robin's screen, facing in. Robin is
     walked in by the controls' override (*The controls*), and another
     character's record (`0xB7BD`) is set up and drawn. Where it stands
     depends on bit 7 of a byte for each location at `0x7AC2`. The tour
     reaches this, but never with that bit set.
  4. The floors are updated.
  5. Each character in Robin's column, or one beside it, is put on its
     floor (`0:DCDC`). Its position is rounded down to a multiple of 4. If
     something is in the way there (`0:DD3D`), it goes to the first clear
     place from the left, or to `0x64` if there's none.
  6. Their sprite records at `0xAAB8`, 11 bytes apart, are given their
     animation sequences: walking left (`0xAAF6`) or right (`0xAB01`), or
     standing still (`0xAB24`). Bit 6 of each state becomes a copy of
     bit 7.
  7. The four are moved once each (`0xA8D6`, four times).
  8. Everything else that moves is reset:
     - the objects in flight (`0xBE3F`–`0xBE46`, #40);
     - the wanderer (#39);
     - for the locations from 256 up, a second group of four characters,
       with their records at `0xDBF1` and their lists at `0xDC2B`, one for
       each row (#42).
  9. Robin's sprite is drawn.
- **Moving them** (`0xA8D6`, from the main loop) takes one character a
  frame, cycling through the four with a counter kept in its own code
  (`0xA8D7`). A character moves only if it isn't standing still and is in
  Robin's column or one beside it:
  - if anything is in the way under it, it turns round;
  - otherwise, one time in 16, it turns to face Robin's position on his
    floor: R decides (`0xA969`);
  - then it steps 1 along its screen, and past an edge into the next
    column.
- **Drawing them.** A character in Robin's column is drawn at its
  position. One in the column to the right, within `0x10` of its left
  edge, is drawn at its position plus `0x70`, about to walk on. Anything
  else is erased.
  - Just after it turns, it's given a turning sequence (`0xAB0C` right,
    `0xAB10` left), and bit 6 catches up with bit 7.
  - With bit 5 set, it's given the sequence for stopping (`0xAB20`), and
    bit 5 becomes bit 4.
  - It's drawn from frame table `0x8C69` (`0:C7EF`, *Sprites*).
- **Firing.** A character that faces Robin from at least `0x30` away (96
  pixels) may fire. It always does while Robin has the bow (`0xD47B`); otherwise R
  decides, one time in 2 (`0xAA34`). The shot goes into a slot of the
  objects in flight (`0xBE41`, two bytes each), if that slot is free (#40).
  The slot is 1 for the first of the four, 2 for the second, and 0 for the
  other two, which share it. The shot gets the character's position and its
  way, and the character the sequence for firing (`0xAB14` right, `0xAB1A`
  left).

### The second group

Found by reading `0:DAE4` (#41).

**Rewritten (`games/robin/src/characters.rs`, `move_second`) and
confirmed** against the original by `tools/robin-verify`: the tour and play
reach every instruction. **confirmed**, unless marked.

- **The locations from 256 up have a second group of four characters**,
  with their own lists at `0xDC2B`, 12 bytes each, one for each row by its
  number (*The four on each row* for the entry's set-up). The current row's
  list is `0xC446`, and their sprite records are at `0xDBF1`, 11 bytes apart,
  drawn from frame table `0x8C8F`.
- **Walking them** (`0:DAE4`, from the main loop) is the four's walking
  (`0xA8D6`) with less to it:
  - one a call, by a counter kept in its own code (`0xDAE5`);
  - only one in Robin's column or beside it moves;
  - it moves two positions a step, with no floor to stop it, and is never
    still;
  - one time in 32, R (`0:DB38`) turns it to face Robin. It judges the way
    from the start of the character's screen, not its position: right if
    Robin's position, a quarter of it plus `0x1C`, is at least `0x1C` times
    how many screens the character is along;
  - it's drawn as the four are. When it's erased, it's given its walking
    sequence again (`0xDC1D` left, `0xDC24` right).
  - it never fires; touching Robin hurts him instead (*Fighting*).

### The fifth character

Found by reading `0xB7DB` and the code it reaches (#41).

**Rewritten (`games/robin/src/fifth.rs`) and confirmed** against the
original by `tools/robin-verify`. The robbery is reached by a supplement
game that puts the first of the pair in front of Robin, armed (#53). Only
its leaving once Robin is off its row is never reached. **confirmed**,
unless marked.

- **Two who walk a route together**, whose records are extended sprite
  records:
  - the first at `0xAAE4`, the fifth record a hit can strike (*Fighting*);
  - its companion at `0xBA4F`.

  After each record come its way (`+0x0B`, bit 0 set for left), the route's
  start and end locations (`+0x0C`, `+0x0E`) and the location it's in
  (`+0x10`). `0xBA61` says who's out (bit 1 the first, bit 0 the companion)
  and whether the first has been robbed (bit 2).
- **Bringing them on** (`0xB7DB`, from the main loop): when neither is out,
  and Robin hasn't robbed the first six times (`0xD482`), R picks one of
  eight routes at `0xBA63`, never the same start twice running. Both start
  at its start, the first at position `0x18`, the companion at `0x30`.
- **Walking them**, every fourth call (a counter, `0xBA62`, and a mask kept
  in the code, `0xB854`), each that's out:
  - steps one position its way, into the next location along at an edge,
    and turns round at either end of its route;
  - is drawn, as the four on each row are, from its own frame table: if it's
    in Robin's location, or coming on from the next one to the right.
- **The walking is parameterised by its own code** (`0xB947`): the two
  sequences, the frame table, and the target of a jump (`0xB953`), which
  makes it turn on the spot instead of walking.
- **Robbing the first**: when Robin strikes it (bit 5 of its flags), it's
  marked robbed, and drops what it carries, one or two kind-2 items, where
  its companion is (*Items*: the dropping code's operands are pointed at the
  companion's position, then back at Robin's). Each robbery counts up to
  six. From then on, while it's on Robin's row it only turns on the spot,
  every second call; once he's off its row it's gone. The companion goes
  too, once it's away from Robin and inside the screen's middle.

### The scripted scene

Found by reading `0xB723` and the code it reaches (#42).

**Rewritten (`games/robin/src/scene.rs`) and confirmed** against the
original by `tools/robin-verify`. Play reaches every part of it: it enters
the scene's location, `0x43`, near frame 9,600. The end reads the ROM
(below), whose bytes the checks give the rewrite (#21). **confirmed**.

- **One character stands at a location** picked when a game starts (`0xD295`,
  one of eight at `0xD27F`). Entering it sets up its record at `0xB7BD`
  and walks Robin in (*The four on each row*, step 3).
- **Each call** (`0xB723`, from the main loop) at that location counts down
  `0xD297`, and acts every fourth:
  1. the character is drawn, from frame table `0x8C8F`;
  2. if Robin is in the middle of the screen (his position from `0x24` to
     `0x5B`) and it hasn't yet moved, it starts: bit 7 of its flags is
     set, the counter starts again at `0x50`, and it's given a sequence by
     the way it faces (`0xB7C8`, or `0xB7CD` with bit 7 of its `+8`);
  3. otherwise, once its sequence reaches its frame 2, Robin stops being
     walked in (the controls' override, *The controls*), and when the
     counter has run out, the end.
- **The end** (`0xB790`): a sample (`4:C000`,
  *Sound*), the warble at `0x8B32` (*What it uses from the ROM*). Then Robin
  is at location `0x9C`, entered as a new game's first location is: the
  play area cleared, drawn, recoloured (`0:C306`), the characters' entry,
  and the reveal (`0:CD73`, *The screen*). Last, the scene moves on to a new
  place (`0:CE8D`, from `0:CD5F`'s random value, #21). It returns to the
  main loop's start, not its caller.

### The wanderer

Found by reading `0xBA83`, `0xBD87` and the code they reach (#39).

**Rewritten (`games/robin/src/wanderer.rs`) and confirmed** against the
original by `tools/robin-verify`. Play and the control-method games walk it
and erase it; the tour draws it, and once meets it, which reaches every
instruction of the meeting. **confirmed**, unless marked.

- **A friendly character** that walks the forest's rows from location to
  location. Meeting it restores some of Robin's energy.
- **Its record** is a sprite record at `0xBB1B`, drawn from frame table
  `0x8C8F`. Its state follows:
  - its location, a word at `0xBB26`;
  - a timer, `0xBB28`;
  - bit 6 of `0xBB29`, set once Robin has met it;
  - its position within its location, `0xBB2A`, from 0 to `0x6F` as the
    four characters' (*The four on each row*).
- **When a game starts** (`0:CEA1`), it's put at the special location kept
  at `0xD291` (*The map*), at position `0x38`, with its timer at `0x70`,
  not yet met, walking right.
- **Walking** (`0xBA83`, from the main loop) acts every fourth call, by a
  counter kept in its own code (`0xBA84`). When it acts:
  1. **The timer counts down.** When it runs out, it starts again at
     `0xE0` and the wanderer turns round, with the animation sequence for
     the new way (`0xBB2B` right, `0xBB37` left), and bit 7 of `0xBB29` is
     set.
  2. **It steps one position** that way. Past `0x6F`, or below 0, it goes
     into the next location along: its location word moves on by one, and
     its position wraps by `0x70`.
  3. **It's drawn** if it's in Robin's location, or in the next one with
     its position below `0x10` (drawn at the position plus `0x70`), as the
     four characters are. Otherwise it's erased.
- **Its way is kept in the code.** The step is one instruction, at
  `0xBAB6`, that either adds 1 to its position or takes 1 away. Turning
  round flips the instruction: bit 0 of its opcode switches between
  `INC (HL)` and `DEC (HL)`. The set-up clears the bit, for right.
- **Its animation starts only when the timer first runs out**, `0x70` of
  its steps into a game. Until then its sequence is 0, and drawing it reads
  the ROM (*When a sprite reads the ROM*, #21).
- **Meeting it** (`0xBD87`, from the main loop): if it's drawn (bit 0 of
  its record's flags) and not yet met, and overlaps Robin, within `0x0C`
  across and `0x20` up and down (`0xBDB9`, a test the objects in flight
  share):
  - a sound starts (`6:C00C`, through the trampoline, *Sound*): `6:C139`
    sets a flag (`0xC15A`) and two AY registers for the interrupt's music
    player, unless one is already playing (`0xC190`);
  - the play area flashes: its attributes' ink goes up by one, 16 times over
    (`0xBDD3`), checked picture by picture (#57). Its count runs from
    `0x240` down past 0, so it moves one cell more than the play area, the
    first of the lower panel's, whose ink comes back round after the 16;
  - the lower panel's colours are reset (`0xBE0F`, *Robin's actions*);
  - his energy goes up (`0:D7F7`, *Robin's actions*);
  - it's marked met, so it happens once a game.

## Items

Found by reading `0:D484`, `0:D6D0` and the code they reach (#41).

**Rewritten (`games/robin/src/items.rs`) and confirmed** against the
original by `tools/robin-verify`. Play and the tour place and restock them;
the collector's game, a supplement, starts Robin beside seven of them with
his inventory full, which reaches picking up each kind, carrying, and the
drop. Four instructions of picking up are never reached: taking one of the
thirty (play never walks him into one), the eleven full when he drops one,
and a drop at the screen's left edge. **confirmed**, unless marked.

### Where they are

- **Items are messages printed on the screen.** An item's kind is the stock
  message that draws it (*The text printer*), and the printer's list of
  recorded messages (`0xD457`) is what's on the screen now.
- **Thirty fixed places in the world** at `0xD38A`, five bytes each: the
  location (a word), the position, and the kind, 0 once taken.
- **Eleven more** at `0xD420`, the same five bytes, free while the location
  is `0xFFFF`: items dropped, and two the game puts there when it starts.
- **On entering a location** (`0:D484`, from the entry):
  1. the world restocks (below);
  2. the list of what's on the screen is cleared;
  3. of the thirty, the first still there at this location is printed;
  4. of the eleven, every one at this location is printed.

  If the eleventh is one, the original takes one return address too many
  off the stack, so it returns to its caller's caller. **read**, never seen.

### Restocking

`0:D2DF`, first in entering a location, if the last item taken was of kind
5 or 7 (`0xD483`). Its test reads the Z flag after a load, which sets no
flags, so whether it acts depends on the flags it's called with.
- R picks a number: 6 to 10 for kind 5, 8 to 12 for kind 7.
- While the world has fewer than that of the kind (counted at `0xD47F` and
  `0xD480`), one of the thirty places, picked from R and the next free one
  on (`0:D35A`), gets that kind.

### Picking one up

`0:D6D0`, from the main loop: the first item on the screen within reach of
Robin is taken. Within reach is its first position byte from `0x08` to
`0x17` less than his horizontal position (at least `0x18`), and its second
from `0x0D` to `0x1C` more than his vertical one. The printer's positions
and his aren't in the same units, so these are as the bytes compare.
- **Arrows** (kind 7) only if he has none left; they give him ten, with a
  beeper sound (`0:D8C6`).
- **Kinds 0, 3, 6 and 7 print a message** first (`0xB351`, `0xB37D`, `0xB3E9`,
  `0xB423`).
- **It's printed again to take it off the screen**, and taken out of the
  eleven, or else out of the thirty. From the thirty, a kind 5 or 7 lowers
  its count and is remembered for restocking; an arrow stops there.
- **What it gives**, by kind, each with a beeper sound:
  - 0: the sword (`0xD47A`), 3: the bow (`0xD47B`), 6: a third (`0xD47C`);
  - 1: energy (`0:D7F7`, *Robin's actions*);
  - the rest go into his inventory.

### His inventory

Eight slots at `0xD472`, `0xFF` where empty. A new item goes in first,
pushing the others along, and the panel shows them, in two rows of four from
`0xD8D6`, as one of two messages (`0xB404` for kind 5). The one pushed off
the end, if it's a kind 2, is dropped where he stands (`0:D874`): into the
first free of the eleven, at his location and just above and behind his
position, and printed if it's on the screen.

`0:D874` takes where to drop from its own code. Its instructions' operands
name the location and the position to read, Robin's (`0xC440`, `0xCB7F`,
`0xCB80`) unless the fifth character's code has set them to its own
(*The fifth character*).

## Trades and journeys

Found by reading `0:DDEF`, `0:C3CA`, `0:D8EC` and the code they reach (#41).

**Rewritten (`games/robin/src/journeys.rs`)**. Confirmed against the
original by `tools/robin-verify` as far as it's reached: the trade's test,
taking out of the inventory and showing it, recolouring, and the doorway,
its sparkle and the hook. Those three read the ROM through `0:CD5F` in
nearly every run, and the checks give the rewrite the bytes the original
read (#21). The trade itself needs R at `0x13` with three kind-2 items
carried, which a supplement game brings about by sending Robin to it with
them (#54). The journeys, their wipe (picture by picture, #57) and taking
the third back are reached by another, which sends him into the doorways
with what each needs (#79). **confirmed**, unless marked.

### The trade

`0:DDEF`, from the main loop, at the first special location (`0xD28F`,
*The map*), once a visit (`0xDED2`, cleared on entering a location):
- **if R is `0x13` and he carries three kind-2 items**, they're taken out of
  his inventory (`0:DA6E`, the rest moving up), the inventory shown again
  (`0:DA84`), and the robberies' count lowered by three (*The fifth
  character*);
- **he's given the sword**, or if he has it, **the bow and ten arrows**, or if
  he has both, **one of three pieces** (`0xD47E`, shown in the panel); each
  with its message;
- **a cell beside the location flashes**: a colour kept in the code
  (`0xDEB7`) is written to it, a busy wait, then 4, which the code keeps:
  `0x12` the first time, 4 for every trade after.

### Doorways and journeys

- **Nine doorways** (`0xDAB9`). On entering one with anything in his
  inventory (`0:D8EC`, from the entry), its attributes are drawn (`0xA2F4`),
  the main loop's hook is set to its sparkle, he's walked in by the controls'
  override (*The controls*), and a sound starts. Then its code runs on into
  the sparkle's first frame.
- **The main loop's hook** (`0:C3CA`) is a call whose operand the game sets:
  to a plain return (`0:DA03`), or to the sparkle (`0:D97D`).
- **The sparkle**, each frame: the doorway's 40 rows of pixels (`0xA204`)
  masked with random bytes are combined with the screen's. That's 120 bytes,
  each with two reads of R, one of them through `0:CD5F`, which reads the
  byte at R × `0x101`, the ROM below `0x4000` (#21). It runs in two passes,
  `XOR` then `OR`, and the code rewrites its own instructions to switch:
  the combining one (`0:D9AA`, `XOR (HL)` or `OR (HL)`), and the one that
  reads R (`0:D997`), which becomes `LD R,A` for the last frame.
- **At the end** (`0:DA04`), the hook is taken out, and what he carries
  decides:
  - with three kind-5 items, they're taken and he's sent to location `0xCC`;
  - with two, they're taken and his health is restored;
  - with one, it's taken;
  - with none, a kind-2 item is taken back, or if there isn't one, he's sent
    to location `0x9C`.

  Then he walks back out, the way he came, unless he's being sent.
- **A journey** (`0:C3CA`, when `0xDAD6` says so) enters the new location
  much as the entry does: the play area cleared, drawn, recoloured
  (`0:C306`), then a diagonal wipe onto the screen (`0:C40F`). The third
  item (`0xD47C`) is taken back if he has it (`0:C3AE`). Then the items,
  the copy to the screen, and the characters' set-up. It ends by jumping
  back to the main loop's start, not returning.
- **Recolouring** (`0:C306`, **confirmed**): at 26 locations of the first row (`0xC448`),
  the attribute buffer's colours become white ink where there's paper, and
  bright cyan where there's ink but not white.

## Fighting

Found by reading the main loop's `0xBB84`, `0xBC0B`, `0xBC6C`, `0xBCAA` and
`0xBD19`, and the code they reach (#40).

**Rewritten (`games/robin/src/fighting.rs`) and confirmed** against the
original by `tools/robin-verify`. Play and the tour reach shots flying and
hitting Robin, his fists and sword striking, and the second group touching
him. The armed game (*Robin's actions*) lands his arrows on a character.
**confirmed**, unless marked.

### The objects in flight

- **Four slots** at `0xBE3F`–`0xBE46`, two bytes each:
  - the column, with the way it flies in bit 7 (set for left), or 0 while
    the slot is free;
  - the row, with two flags: bit 7 set when it's just been fired, bit 6
    when it's hit something.

  Slot 0 is Robin's arrow (*Robin's actions*); slots 1 to 3 are the four
  characters' shots (*The four on each row*).
- **Flight** (`0xBB84`, from the main loop) takes each slot in turn:
  1. its two flags are read and cleared;
  2. unless it's just been fired, it's erased;
  3. it steps one column its way;
  4. if it's hit something, the slot is freed;
  5. otherwise it's drawn, and freed if it's now at the play area's edge
     (column below 2, or from `0x1E`).

  Which of these it does is decided by rewriting the offsets of two of its
  own jumps (`0xBBB4`, `0xBBC5`) from the flags.
- **Drawing one** (`0xBBEC`) XORs a byte of 8 pixels into the top pixel row
  of its cell in the back buffer (`0xEB00` + row × 256 + column), and marks
  the cell changed with `0x46` (*The screen*). Drawing it again erases it.

### Hits

- **A shot hitting Robin** (`0xBC6C`, while he isn't down): a shot whose
  column, times 4 plus 8, is within 8 to the right of his position, and
  whose row, times 8, is within `0x24` below his, is marked hit. He's hit
  (below), and a sample plays (`4:C015`, *Sound*).
- **Robin's arrow hitting a character** (`0xBC0B`): if it's in the rows the
  characters walk (`0x0A`–`0x0D`), and one of five records from `0xAAB8`
  is drawn, not stopped, and within 8 of it the same way, the arrow is
  marked hit and that character struck (below). Then a tune starts for the
  music driver (`6:C048`), and a zap plays there and then (`0xBC4C`): for
  each count from `0x14` to `0x27`, that many writes to port `0xFE`, each
  of R's bits 3 and 4, with a delay that shortens between them. That's 590
  writes, and 590 reads of R.
- **His sword and fists** (`0xBD19`): on the frames of his attack that
  strike (frame `0x1A`, `0x1C` or `0x1F` of his sequence), a point ahead of
  him that overlaps one of the five records (`0xBDB9`, *The wanderer*)
  strikes that character, with the same tune and zap.
- **The second group touching him** (`0xBCAA`, at the locations from 256
  up, while he isn't down): unless a cooldown (`0xBE48`) is counting down,
  one of their four records (`0xDBF1`) overlapping him hits him, plays a
  sample (`4:C012`), and starts the cooldown at `0x1F`.

### Robin hit

`0xBCFB`, unless he's already down:
- **His health is `0xBE47`**, which is also the lower panel's colour (`0xBE0F`,
  *Robin's actions*): the panel shows how he is.
- A hit takes 1 off it. When it's down to 2, he's knocked down instead
  (`0xCB75` = `0x6E`, *Robin's actions*), and the wanderer can be met again
  (*The wanderer*).

### A character struck

`0xBDF2`: the record struck gets bit 5 of its flags set. If it's one of the
four row characters, bit 5 of its state is set too, so it stops (*The four
on each row*).

## The main loop

`0xBE62`, which every routine above is called from, once a frame.
**Rewritten (`games/robin/src/main_loop.rs`, `frame`) and confirmed** as a
whole: a suite runs one pass of it at a time, from `0xBE62` back to
`0xBE62` (#60). **confirmed**, unless marked.

- **BREAK** (`0:C433`), first: Caps Shift with Space starts a new game
  (`0xBE5A`: the stack reset, then the new game's set-up, `0:CC72`). Its
  answer is in the carry, from rotating the keyboard's bits, so the other
  flags are left as they came in.
- **Then, in order**:
  1. Robin's update (`0:C59A`, *Robin's actions*);
  2. the objects in flight (`0xBB84`, *Fighting*);
  3. one of the four (`0xA8D6`), and one of the second group (`0:DAE4`),
     *The characters*;
  4. the hits: arrows (`0xBC0B`), shots (`0xBC6C`), the second group
     touching Robin (`0xBCAA`), and his sword and fists (`0xBD19`),
     *Fighting*;
  5. the scripted scene (`0xB723`), the wanderer (`0xBA83`) and the fifth
     character (`0xB7DB`), *The characters*;
  6. meeting the wanderer (`0xBD87`);
  7. the screen's flush (`0:C754`, *The screen*);
  8. picking things up (`0:D6D0`, *Items*);
  9. the game over (`0xBF3F`), the hook (`0:C3CA`) and the trade
     (`0:DDEF`), *Trades and journeys*;
  10. last, leaving the screen (`0xBE9B`, *Robin's movement*), which takes
      the step and enters the next location.
- **Some passes end early**, back at the loop's start: the scene's end and
  a journey jump there, rather than returning. The game over and BREAK go
  to a new game.
- **The game over** (`0xBF3F`): once his energy is negative and he's lain
  down long enough (`0xCB75` at `0x3C`, *Robin's actions*), it prints its
  message (`0xB51F`) and starts a tune (`6:C006`). Then it waits, with
  interrupts on, until a key is pressed, and starts a new game as BREAK
  does. The rewrite leaves that wait to its caller.

### Entering a location

`0xBECE`, from leaving the screen, with the direction he left by.
**confirmed**
1. The step to the next location (`0:C127`, *The grid*). At `0x69`, the
   ending instead (below).
2. Bit 1 of Robin's flags is cleared, and the play area cleared and drawn
   (`0:CF19`, `0xBF6A`), with the special locations' lists (`0:C056`).
3. At location `0xBB`, every cell of the attribute buffer with any paper
   becomes green ink on black (`0:C33C`).
4. The first row's recolouring (`0:C306`, *Trades and journeys*).
5. A message at a few locations of the first row (`0:C35A`). Without the
   third item and with fewer than three pieces, stock message `0x4E` at
   `0x65` and `0x6E`. Otherwise `0x4D`, at `0x79` with all three pieces or at
   `0xBB`. At `0x6E` the third item is taken back (`0:C3AE`) instead.
   Whatever it prints, the printer's replace flag is left at the message's
   number.
6. The characters' entry (`0:C16E`), the items (`0:D484`), and the whole
   play area copied to the screen (`0:C086`, `0:C6FE`).
7. The hook set to nothing (`0:DA03`), then the doorway (`0:D8EC`).

**A quirk the rewrite copies**: placing the items restocks only if the Z
flag it's called with is clear (*Items*, *Restocking*). Here that's the flag
the characters' entry leaves, which ends by drawing Robin. For a pixel
frame, the frame drawing (`0:C5CE`) returns with AF taken from a word it
pushed, a pointer into the frame. So Z is bit 6 of the low byte of his
frame's address plus 2. The address comes from the frame table indexed by
the frame doubled in one byte, so the frame's bit 7, the way he faces,
drops out. A character figure (the frame's header with its low nibble 4 or
more) is drawn by `0:C47D`, which returns straight to the caller after a
`DEC C` reaching 0, so with Z set. Play and the tour never entered a
location with Robin in a character figure; the traveller's game did (#8).

### The ending

Entering location `0x69` (`0xBEDF`), the only location not entered by the
entry above (*The grid*). **Rewritten (`main_loop::ending`) and confirmed**
up to its waits: play never gets there, so a supplement game, the ending's,
sends the original into it from `0x68` through its own step (#60).
**confirmed**, unless marked.
1. `0:CF5F`: the play area cleared, stock message `0x50` printed at its top
   left, and revealed (`0:CD73`, *The screen*).
2. With interrupts off, the border flashes: R is written to port `0xFE`
   `0x960` times.
3. It waits for every key to be let go, a delay, then for any key, and
   starts a new game (`0xBE5A`). The rewrite leaves these waits to its
   caller. **read**

## A new game

`0:CC72`, from the hand-over and wherever a game ends (`0xBE5A`).
**Rewritten (`games/robin/src/new_game.rs`) and confirmed**, piece by piece
(#61). **confirmed**, unless marked.

### The menu

- **The menu** (`0xAB28`): the play area cleared to white ink on black
  (`0x47`), stock message `0x40` as a highlight beside the chosen method's
  line (at `0x10` across, and `0x20` times the method plus one down; the
  method is kept at `0xABEE`), its five lines (`0x48` to `0x4C`), then the
  reveal (`0:CD73`, *The screen*). It jumps into the reveal rather than
  calling it.
- **Shown** (`0:CCAB`): the menu, then a sample (`4:C00F`, *Sound*) and the
  menu's tune (`6:C003`: speed 6, its parts at `6:C4EC` and `6:C37F`).
- **Its keys**, polled with interrupts on, in a busy loop (`0:CCBB`): 0
  starts the game; any of 1 to 4 is a pick (`0:CFB2`), and the menu is shown
  again. The rewrite polls once a frame, which sees every press the busy loop
  does, since the window's keys change once a frame.
- **The pick** (`0:CFB2`): 1 redefines the keys, then sets the keyboard; 2
  sets Kempston, 3 Interface II, 4 nothing. Setting a method keeps its
  number at `0xABEE` and its table of ports and bits at `0xD152` (`0xD06E`,
  `0xD07F`, `0xD088`, *The controls*).

### Redefining the keys

`0:CFDC`, from the pick's 1. Its waits are busy loops; the rewrite steps
them once a frame, as it does the menu's.
1. **Its screen**: the play area cleared to `0x45`, revealed, then a
   heading (`0xD0E8`) and the first of five prompts (`0xD10C`, `0x0E`
   bytes apart), printed replacing what's there (`0:CEBB`).
2. **For each of the five**: it waits for every key to be let go (`0:D028`,
   reading every half-row at once through port `0x00FE`), then for exactly
   one to be held (`0:D01D`). It records the key's code at `0xD154` on, puts
   its name from the table at `0xD159` into the prompt (at `+0x0B`), and
   prints the prompt again. Then the next prompt.
3. **Then the keyboard is the method** (`0:CFBD`).
- **The scan** (`0:D033`) reads the eight half-rows from `0xFEFE` to
  `0x7FFE`. A key's code is `0x27` for the first key of the first half-row,
  down by one a key: `0x2F` less the half-row's number, less 8 for each key
  up to and including it. It answers in D (`0xFF` for none) and the Z flag,
  which is clear if more than one key is held. Then D is what it had when it
  found the second.

### The start and the set-up

- **Once, first** (`0:CC72`): the AY's registers reset from the music
  player's table (`6:C045`), the mirror and row tables (`0:CEC8`,
  `0:CEDE`), the interrupt's table and vector (`0:CF6F`: `0xE200` to `0xE300`
  all `0xFF`, I `0xE2`, mode 2, and the jumps at `0xFFFF` and `0xFFF4` to
  `0:DED3`, *Interrupts*), the lower panel cleared, five strings printed,
  the border black, and two of Robin's counters cleared. Then the menu.
- **A new game's set-up** (`0:CCD2`), once 0 is pressed. It's checked
  whole, and in pieces; two of them read the ROM in nearly every run, whose
  bytes the checks give the rewrite (#21):
  1. the AY reset, the lower panel cleared, Robin's record from its
     template (`0xCB87`), his health's colour full (`0xBE47`), a flag
     (`0xCB85`), and the hook cleared;
  2. sixteen three-byte entries at `0x8B02` from `0:CD5F`'s random
     values: one below 16, one whole, and one's bit 6 as bit 7;
  3. the special locations and the start (`0:CE4B`, below), the scene's
     place (`0:CE8D`), the wanderer (`0:CEA1`), a title (stock message
     `0x4F`) revealed, then the first location drawn, its items afresh
     (`0:D298`, below) and placed, and the characters' entry;
  4. the warble (`0x8B32`, *What it uses from the ROM*), its times
     checked too (*Sound*);
  5. the reveal, no cell marked changed, and the arrival's tune.
- **The special locations and the start** (`0:CE4B`), each by R: the
  trade's, one of four bytes at `0xD26B`; the wanderer's, one of four words
  at `0xD26F`; and the start, one of four at `0xD277`. The third special is
  the start, but `0xD2` for `0x9C`.
- **Every item afresh** (`0:D298`): the thirty places emptied, what he has
  and the counts cleared (`0xD47A` to `0xD483`), the fifth character not
  out, the eleven and the inventory emptied. Then energy at the first place
  and at five more free ones (`0:D35A`), the kinds 5 and 7 restocked
  (*Restocking*), and the two the game starts with (`0:D331`). It ends with
  the Z flag set, so placing the items next doesn't restock.

## Sound

- **It uses the AY sound chip.** It selects registers through `0xFFFD` and
  writes them through `0xBFFD`, every frame in play: registers 0 to 10 and 13
  (tone periods, noise, mixer, volumes, envelope shape). It also reads
  registers back through `0xFFFD`. The code is at `0xC0xx`–`0xC2xx`, in a
  paged bank. **provisional**
- **It plays sampled sounds through the beeper and the AY at once**: a
  player in bank 4 at `0xC090`, reached through a trampoline in bank 5,
  `0x5B8A` (**read**, #36):
  - **The trampoline** takes the address and the bank to call from the
    three bytes after its call. It pages that bank in at `0xC000`, keeping
    the one it replaces at `0x5BC8` (and the current one at `0x5BC7`), and
    pages it back afterwards. It leaves interrupts off.
  - **The player** turns interrupts off and saves the AY's registers 14
    down to 1 by reading them back, then sets them all to one value (the
    byte at `0xC1A2`; the table's pointer never moves on), leaving register
    1 selected. It plays a sample of its length plus one bytes, a bit at a
    time, from its highest bit, rotating each byte in place. Each bit is
    masked with an amplitude, which changes every 64 bytes from a second
    table, and the result is written twice: to port `0xFE`, as EAR and MIC
    (bits 4 and 3), and its low four bits to `0xBFFD`, the register left
    selected (1, a tone's coarse period). A delay between bits sets the
    rate. Then it restores the registers and turns interrupts back on.
  - **When Robin is knocked down**, `4:C012` picks one of four samples by R
    (#37), of 1,000 to 2,100 bytes: tens of thousands of writes,
    with the game standing still while they play.
  - The menu plays its samples through the same player (`0xC0AE` is its
    write to port `0xFE`).
- **The beeper's timing** (#67, **confirmed**). A sound on the beeper is
  its writes to port `0xFE` and the time between them, set by the delay
  loops its code runs. The rewrite keeps a clock in T-states, without
  contention, and its sounds move it on as the original's instructions take.
  The suites compare each write's time from the first, exactly, in the
  beep (`0:D8C6`), the twang (`0xBE2C`), the zap (`0xBC4C`, in the hits that
  set it off), the sample player (`4:C090`) and the ending's border flash.
  Between two writes:
  - the beep and the twang: a delay loop of 16 T-states a turn, from the
    count, then 30 more for the beep and 41 for the twang (the count is the
    number of turns, 0 for 256);
  - the sample player: 13 T-states a turn of its delay (`4:C141`), and 244
    more within a byte, 239 to the next byte, 246 to the next 64;
  - the ending's flash: 46.

  On the 128K an `OUT` to `0xFE` is slowed by a few T-states while the
  beam is in the picture. That contention is left out, under 1% of a
  sound's length.
- **The music player**, every frame from the interrupt (*Interrupts*).
  **Rewritten (`sound.rs`) and confirmed** (#62), every instruction reached:
  - **ENTER** (bit 0 of `0xBFFE`) toggles the music (`6:C2CD`, `0xFF` off),
    once a count (`6:C2CE`) set to `0x32` at each toggle has run out. Back
    on, the tune starts again from the entry kept at `0xCBD9`. Off,
    registers 8 and 10 go to 0.
  - **While it's on, the tune** (`6:C0A2`): a count kept in its code
    (`6:C0A3`) runs down from the tune's speed (`6:C0A1`), and at each
    end each part plays its next note. The first (`6:C9A0`) on tone A,
    volume 12, its notes' bit 7 ignored. The second (`6:C9A2`) on tone C with
    the envelope. A note is a period from the table at `6:C308`. 0 is a
    rest, and `0xFF` loops the part to the address after it: 3 bytes after
    for the first part, straight after for the second.
  - **The wobble** (`6:C26F`): tone C's period read back from registers 4
    and 5, a step kept in its code (`6:C27E`) added, and written back.
    Every fourth frame (a count in its code, `6:C293`) the step changes
    sign, as the code works it out: the low byte negated and the high byte
    inverted, which is a true negation only when the low byte is 0.
  - **The effect** (`6:C1EB`): while its count (`6:C190`) runs, tone B at
    full volume, its period (`6:C18C`) swept by a step (`6:C18E`), the
    low byte's top bit flipped first each frame. Otherwise (`6:C15B`), with
    no meeting's sound (`6:C15A`), the mixer is set and tone B quiet. With
    it, its count runs down, and every eighth frame tone B's volume is the
    count over 8.
- The menu offers *ENTER = music on/off*, and ENTER toggles it at any
  time, in play too.

## Input

- The menu offers keyboard, Kempston and Interface II (*A new game*).
- In play, with the keyboard, it reads the half-rows `0xF7FE` (1–5), `0xFBFE`
  (Q–T), `0xFDFE` (A–G), `0xFEFE` (Shift–V), `0xBFFE` (Enter–H) and `0x7FFE`
  (Space–B). It does not read the row with O and P. **provisional**

## What it uses from the ROM

**In the checks, the ROM's bytes the original read are given to the
rewrite**, as R's values are (#21, #37 Decision 2): `robin::Game::rom`,
empty in play, where a read below `0x4000` gives 0. So no call is skipped
for reading the ROM any more, only counted.

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
    (called from the scripted scene's end, `0xB796`, and the new game's
    set-up, `0xCD4E`; rewritten as `sound::warble`, #42)
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
