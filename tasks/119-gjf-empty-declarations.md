# Format empty declarations as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

Cleanups (task 112) delete most redundant semicolons, but an enum constant's body keeps its first `;`; GJF prints it through `dropEmptyDeclarations`.

## Done when
- The visitor ports `dropEmptyDeclarations` and no longer defers `EmptyDecl`
- `cleanup-enum-constant-body` passes with its `skip` marker deleted
