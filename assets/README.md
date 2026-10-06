# Assets

This project contains no part of the original game. Put your own copies of
these files here (everything in this directory except this README is
ignored by git):

| File | What | Needed by |
|------|------|-----------|
| the tape | Robin of the Wood, the 128K release, `.tzx`, SHA-1 `2aad3402cdc08907000900c5da8c29eeb2f48c9e` | the game |
| `128.rom` | ZX Spectrum 128K ROM, 32K: the 128 editor ROM then the 128's own 48 BASIC ROM (which is not `48.rom`), SHA-1 `16375d42ea109b47edded7a16028de7fdb3013a1` | development tools only (the reference machine) |
| `48.rom` | ZX Spectrum 48K ROM, SHA-1 `5ea7c2b824672e914525d1d5c419d71b84a426a2` | development tools only (the 48K reference machine) |
| `tests.in`, `tests.expected` | The Fuse project's Z80 test corpus | development tools only (the processor conformance test) |
| `z80full.tap`, `z80ccf.tap`, `z80memptr.tap` | Patrik Rak's z80test, v1.2a | development tools only (the processor conformance test) |
| `minfo.tap`, `timingtest.tap` | Patrik Rak's zxtests `minfo` and Timing Test v0.3 | development tools only (the machines' timing) |
| `fusetest.tap` | Philip Kendall's Fuse Test (2008-03-28) | development tools only (the machines' timing) |
| `butler-128k.szx` | Richard and Tim Butler's ZX Spectrum Timing Tests 128K v1.0 | development tools only (reported, not judged) |

The tape is the 128K release as a TZX file, every block at the ROM's standard
speed (`docs/re/robin.md`, *The tape*). Its name does not matter; its SHA-1
does. It is pinned in `games/robin` (`robin::assets::TAPE_SHA1`). The game
refuses any other file, naming its SHA-1 and the supported one, and
`cargo test` fails if the tape here is a different one.

**Only the tape will be needed to play.** Everything else is for the tools
that run the original for comparison and check the interpreter; you can
ignore them unless you are working on the code.

## Where to get them

**The game.** [World of Spectrum](https://worldofspectrum.net/) and
[Spectrum Computing](https://spectrumcomputing.co.uk/) keep Spectrum software
available and remove titles whose rights holders object. That a title is
listed means nobody has objected, not that its rights holders gave
permission. If you own the game on tape, a dump of your own copy works as well,
provided it is the same dump.

**The ROMs.** Amstrad bought Sinclair's computer business in 1986 and gave
permission for the Spectrum ROMs to be redistributed with emulators, so long
as the copyright notice is kept and they are not sold; the rights passed to
Sky when Amstrad was bought in 2007. That permission is why emulators ship
the ROMs, and the easiest legitimate sources are:

- [Fuse](https://fuse-emulator.sourceforge.net/), which includes `48.rom`, and
  the 128K's two halves as `128-0.rom` and `128-1.rom`
  (`cat 128-0.rom 128-1.rom > 128.rom`)
- the `spectrum-roms` package in Debian and Ubuntu
- World of Spectrum, which also hosts them

This is an informal permission rather than a formal licence, but it is the
basis emulator projects and Linux distributions have relied on for years.

**The Z80 test corpus.** Two text files from the
[Fuse](https://fuse-emulator.sourceforge.net/) project, stating for 1335
cases what a Z80's registers, memory and T-state count should be after
running. They are GPL-licensed, which is why they are fetched rather than
copied in here:

```sh
base='https://sourceforge.net/p/fuse-emulator/code/HEAD/tree/trunk/fuse/z80/tests'
curl -L -o assets/tests.in "$base/tests.in?format=raw"
curl -L -o assets/tests.expected "$base/tests.expected?format=raw"
```

**z80test.** Patrik Rak's Z80 tests, whose expected results were taken on a
real Spectrum. MIT-licensed, but they are tapes, which this repository never
carries, so they are fetched too:

```sh
curl -L -o z80test.zip https://github.com/raxoft/z80test/releases/download/v1.2a/z80test-1.2a.zip
unzip -j z80test.zip z80test-1.2a/z80full.tap z80test-1.2a/z80ccf.tap z80test-1.2a/z80memptr.tap -d assets
```

**The timing tests** (`crates/zx-runtime/tests/hardware.rs`) run the
machines against programs whose results were measured on real ones. They
need the ROMs, so they run only locally. Rak's and Kendall's are GPL, from
their authors' or a mirror's archives:

```sh
curl -LO http://zxds.raxoft.cz/taps/misc/zxtests-3p.zip
curl -LO http://zxds.raxoft.cz/taps/misc/timingtest-0.3.zip
curl -L -o fusetest.zip 'https://zxe.io/depot/software/ZX%20Spectrum/Fuse%20Test%20(2008-03-28)(Kendall,%20Philip)%5B!%5D.zip'
unzip -j zxtests-3p.zip minfo.tap -d assets
unzip -p timingtest-0.3.zip timingtest-0.3/timing.tap > assets/timingtest.tap
unzip -p fusetest.zip fusetest/fusetest.tap > assets/fusetest.tap
```

Butler's tests carry no licence or terms, so they are never fetched
automatically; keep your own copy as `assets/butler-128k.szx` (the
`ZX Spectrum Timing Tests - 128K v1.0` snapshot on zxe.io).

Without the corpus, z80test or the timing tests, `cargo test` says those
checks were skipped, and every other check still runs.
