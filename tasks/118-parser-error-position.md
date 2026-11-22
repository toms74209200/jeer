# Report syntax errors at javac's position

Bounded context: `parser` (`src/parser.rs`, `src/parser/`).

javac reports some syntax errors at another position than jeer: `';' expected` at the end of the previous token (`reject-missing-semicolon`: the oracle says line 2, jeer line 3), and `reached end of file while parsing` at the last token (`reject-unbalanced-brace`: the oracle says line 4, jeer line 5).

## Done when
- The parser places these errors where javac does
- `reject-missing-semicolon` and `reject-unbalanced-brace` join the small test of task 118-gjf-diagnostic-position that checks each oracle-rejected fixture's `L<line>`
