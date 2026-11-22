# Add google-java-format's cleanups

`Cleanups` / `CleanupScanner` (redundant semicolons, empty annotation parentheses, single-parameter lambda parentheses, and the other passes Spotless's default enables) and `ModifierOrderer`, applied before formatting as `Formatter.formatSource` does.

Until then the visitor returns `Unformatted::Deferred` (the input is kept as it is) for what these passes would change: empty declarations and statements (`;`), `@A()`, `enum E { ; }`, and modifiers out of JLS order.

## Done when
- A spec-change commit adds fixtures of our own for each cleanup pass, with `skip` markers; the implementation deletes them
- Small tests cover each pass alone
- The visitor no longer returns `Unformatted::Deferred` for the constructs above
