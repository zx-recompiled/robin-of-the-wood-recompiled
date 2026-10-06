# Robin of the Wood: reverse-engineering notes

What is known about the original program, in our own words: descriptions,
addresses and layouts, never listings of its code.

Each fact is marked with how it is known:

- **confirmed**: seen running the original in this project's reference
  machine;
- **provisional**: seen only in an emulator whose accuracy is unchecked (zx84);
  to be re-confirmed in the reference machine;
- **read**: understood from the disassembly;
- **guess**.

## The tape

To be established (ticket #1): the dump's SHA-1, its format (`.tzx`) and
blocks, and whether its loader is the ROM's or the game's own.

## The 128K machine

To be established (ticket #1): whether and how the game pages memory (port
`0x7FFD`), whether it moves the screen to bank 7, and whether it uses the AY
sound chip (ports `0xFFFD` and `0xBFFD`).

## Where the program starts

To be established (ticket #1): where control passes to the game, with the
stack and the paging state at that moment.

## What it uses from the ROM

To be established (ticket #1): every point where control passes from the
game's code into the ROM while it plays, which ROM is paged in at the time,
and the system variables those routines keep that the game reads.
