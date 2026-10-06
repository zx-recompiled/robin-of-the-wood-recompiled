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

Nothing here is confirmed yet: the reference machine cannot run a 128K until
#2. Everything was read from the tape's bytes, then watched in zx84 running
the same 128K ROM (`assets/128.rom`, whose SHA-1 matches zx84's own).

## The tape

- **The dump**: a TZX 1.10 file, SHA-1
  `2aad3402cdc08907000900c5da8c29eeb2f48c9e`, 62,599 bytes. **read**
- **Every data block is at the ROM's standard speed** (TZX block `0x10`). There
  are no turbo, pure-data or custom blocks, so a reader needs only standard
  blocks, plus skipping the archive-info block (`0x32`) at the start. **read**
- The blocks, in order: **read**

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
  rest are never reached. **read**, and the hand-over **provisional**
- **It enters the ROM loader at `0x0563`, not at `LD-BYTES` (`0x0556`).** It
  does `LD-BYTES`'s first few instructions itself and jumps past the rest of
  its preamble. `0x0563` is the operand byte of an `IN A,(0xFE)`, so the ROM
  resumes on a different instruction from the one its authors wrote. This
  skips setting the border, reading the EAR level, and pushing the address
  of the routine that finishes a load. So `LD-BYTES` returns straight to
  `r1`, with no BREAK check and interrupts left disabled. A loader trap
  placed on `LD-BYTES` would never fire; it has to be at `0x0563`. **read**
- Where each block lands, with the value written to `0x7FFD` first: **read**

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
  **provisional**

## Where the program starts

- `0x5B00` writes `0x10` to `0x7FFD` again and jumps to `0xBE4A`, the
  program's own start. That disables interrupts, sets its stack to `0x5B8A`,
  initialises, and enters its main loop. **read**
- **Entry state for the checks**: PC `0x5B00`, SP `0x508C`, interrupts
  disabled, IM 1, `0x7FFD` = `0x10`, with banks 0, 2, 4, 5, 6 and 7 loaded as
  above. Banks 1 and 3 are never loaded. **provisional**

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
  **read**, the handler's address **provisional**
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

- **No ROM code ran** in six traced frames (about 53,000 instructions), at
  the menu and in play, and no instruction in them names a ROM address as
  data. The only use of the ROM seen is the interrupt's jump reading its
  first byte. Six frames is a small sample: the reference machine should
  count every execution and read below `0x4000` over long runs (#3).
  **provisional**
- IY is not BASIC's `0x5C3A` in play (`0xFF20` was seen), which fits a game
  that calls nothing in the ROM that needs the system variables.
  **provisional**

## Credits and date

- The program's own menu says "© 1986 Odin Computer Graphics". **provisional**
- The individual credits (Steve Wetheril, Paul Salmon, Fred Gray) come only
  from the dump's archive-info block so far, not from the inlay or the
  program.
