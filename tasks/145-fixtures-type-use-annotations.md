# Add fixtures for well-known type-use annotations

Bounded context: `tests/fixtures/` (no `src` change), a fixtures task for task 146-gjf-type-use-annotations.

## Done when
- Oracle fixtures, each with a `skip` marker until task 146, cover `@Nullable` and `@NonNull` imported from `org.jspecify.annotations` and from `org.checkerframework.checker.nullness.qual` on a method return type, a field, a local variable and a parameter, each of the field, local and parameter both fitting on one line and wrapping past 100 columns, a method with type parameters and an annotation after them, and a same-named annotation from another package that stays a declaration annotation
