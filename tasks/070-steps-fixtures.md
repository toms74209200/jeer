# Add oracle profiles for the import and toggle steps with their fixtures

A spec-change task: no production code.

## Done when
- `oracle/build.gradle.kts` has profiles `import-order` (`importOrder()`), `unused-imports` (`removeUnusedImports()`) and `toggle` (`toggleOffOn()` around `trimTrailingWhitespace()`)
- Each profile has at least 8 fixtures of our own under `tests/fixtures/<profile>/`, with `skip` markers; `unused-imports` covers usages only in Javadoc (`{@link}`, `@see`, `@throws`), static imports, wildcard imports and a type shadowed by a nested class
- `scripts/check-fixtures-oracle.sh` passes
