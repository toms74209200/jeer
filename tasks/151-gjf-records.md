# Format records as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitRecordDeclaration` with components, compact constructors and generic records.

## Done when
- `modern-record`, `modern-generic-record` and `stmt-local-class` pass with their `skip` markers deleted (the last is a local class and a local record, task 124, which also needs `new`, task 131)
