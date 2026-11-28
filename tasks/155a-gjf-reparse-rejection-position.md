# Report a rejection of the gjf step's own output where Spotless does

Bounded context: `gjf` (`src/gjf.rs`, `src/gjf/`), with the parser positions of task 155.

When GJF's formatted output does not reparse (RemoveUnusedImports and ImportOrderer parse it with javac), Spotless reports the line of the formatted text. jeer's gjf step parses its output in `steps::unused_imports::apply(&formatted)` and returns `Position::Offset` into the formatted text, which `pipeline::Located::at` turns into a line and column of the step's input, so the line differs whenever formatting moved text. Found in task 154b: a fence info in markdown Javadoc holding `&NewLine;`, `&#10;`, `&#x0A;`, `&#0000010;` or `&#13;` makes the output fail to reparse; the oracle reports `L5 ... <identifier> expected`, jeer line 6, at a column that follows the entity's raw length (`6:5` for `&NewLine;`, `6:9` for `&#10;`). Sorted after task 155, which makes the parser report javac's position.

## Done when
- Oracle fixtures, added in a fixtures task first, cover a fence info decoding to a line terminator (`&NewLine;`, `&#10;`, `&#13;`), and pass
- A rejection from reparsing the step's own output is reported at a line of the formatted text, as Spotless reports it
