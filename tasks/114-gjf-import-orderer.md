# Add google-java-format's ImportOrderer

Spotless calls `ImportOrderer.reorderImports(text, GOOGLE)` after `RemoveUnusedImports`; it is not Spotless's `importOrder()` step (task 080).

## Done when
- Every `imports-*` fixture passes with its `skip` marker deleted
- Small tests cover static and non-static groups, comments attached to imports, and the cases where GJF leaves the import block as it is
