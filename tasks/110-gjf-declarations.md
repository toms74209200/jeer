# Format declarations as google-java-format does

Package, imports as GJF prints them, classes, interfaces, enums, records, fields, methods, constructors, annotations on declarations, modifiers order, blank-line rules between members.

## Done when
- Every `decl-*` fixture passes with its `skip` marker deleted
- The dev corpus match rate (`scripts/corpus-diff.sh dev`) is higher than at the start of the task
