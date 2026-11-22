# Format imports in the gjf profile

Bounded context: `gjf` (`src/gjf/`).

Spotless's GJF step runs `formatSource`, then `RemoveUnusedImports`, then `ImportOrderer.reorderImports(text, GOOGLE)` (not Spotless's `importOrder()`, task 080). The visitor defers every file with an import until then.

## Done when
- The visitor prints package and import declarations as `visitCompilationUnit` and `visitImport` do, and no longer defers imports
- `gjf::step` runs RemoveUnusedImports (calling the `steps` port of task 080 as a `Step`) and a port of ImportOrderer after formatting, in that order, as Spotless's glue does; `pipeline` is unchanged
- Every `imports-*` fixture passes with its `skip` marker deleted, except `imports-javadoc-use`, which waits for Javadoc (task 141)
- Small tests cover ImportOrderer alone: static, module and normal groups, trailing `//` comments, duplicates, and the cases where it leaves the block as it is
