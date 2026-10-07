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

Found by reading Robin's update and the code it calls (#34). **read**,
unless marked.

### Robin

- **Robin is a sprite**: his record is at `0xCB76` (*Sprites*), and his
  position is its next position, `0xCB7F` horizontally and `0xCB80`
  vertically. Next to it are his direction (`0xCB85`) and a state byte
  (`0xCB81`).
- **His update**, `0:C59A`, called from the main loop:
  - it counts down a counter kept in its own code, and acts when it runs
    out;
  - it then resets the counter to 1, so it acts on every call, or to 3,
    every third, while he's fighting (`0xCB74` or `0xCB75` non-zero);
  - when it acts, it moves him (`0:C852`), runs his actions (`0:C8DF`:
    firing and fighting, with the characters), and redraws his sprite from
    frame table `0x8C25`.

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
