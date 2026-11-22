# Format runs of empty statements as google-java-format does

Bounded context: `gjf` (`src/gjf/`), with its fixtures in a spec-change commit first.

GJF's visitEmptyStatement calls dropEmptyDeclarations, which consumes every consecutive `;` at once, and visitStatements calls it once before its loop, ahead of any preserved blank line. jeer prints each `;` of a run as its own statement (task 120), so `if (x) ;;` puts the second `;` at another level, and a blank line between two `;` is kept where GJF drops it.

## Done when
- Oracle fixtures cover runs of `;` in a block, as a statement body (`if (x) ;;`, `while (b) ;;`) and with blank lines between them, in a spec-change commit with `skip` markers
- The visitor prints a run of empty statements as dropEmptyDeclarations does, and the fixtures pass with their markers deleted
