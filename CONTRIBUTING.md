# Contributing

## Run the gate before opening a pull request

```sh
.claude/scripts/check.sh
```

It runs what CI runs and then the differential suites, which CI cannot run:
they need your own copy of the game and the Spectrum 128K ROM, neither of
which is ever committed. Every check must pass, and a check that reports
`no cases ran` counts as a failure, not a pass. Gate on the script's exit
code, not on its output.

## Formatting

The tree is `rustfmt`-formatted, and CI checks it. Run `cargo fmt --all`
before pushing.

Where a table is laid out to be read as a table, a trailing `//` on a line
pins the layout and rustfmt leaves it alone. Use that rather than fighting the
formatter.

## When a lint argues with the code

Much of this source will mirror the original game instruction for
instruction. Some lints will object to that on style grounds: nesting that
follows the Z80 control flow, arithmetic written the way the original writes
it, a match arm per original branch.

When that happens, the answer is a targeted `#[allow(...)]` with a comment
saying which original behaviour it protects — **not** rewriting the logic to
please the lint. A tidier shape that computes something subtly different is a
regression this project cannot afford, and the only thing standing between it
and the rest of the code is the differential suites.

If a lint fix does touch game logic, re-run the gate before pushing, and say
in the commit message that you did.

## Fidelity comes before tidiness

The point of the project is that the rewrite does exactly what the original
does, quirks and bugs included. Two rules follow:

- A "fix" that makes the game behave better than the original is a bug here.
  Reproduce the original, and note the oddity in a comment.
- If a change alters anything the suites compare, the suites decide whether
  the change is right. In starquake-recompiled, the project this one follows,
  they rejected several plausible-looking improvements that were wrong.

## Never commit the game

`assets/` holds your own copy of Robin of the Wood and the Spectrum ROMs, and
is ignored except for its README. CI fails if any game dump reaches the tree.
