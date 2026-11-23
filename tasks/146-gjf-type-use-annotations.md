# Format well-known type-use annotations as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

Port JavaInputAstVisitor's TYPE_ANNOTATIONS, checkForTypeAnnotation and isTypeAnnotation, visitModifiers' split of trailing type-use annotations, and visitMethod's return type annotations (both the type-parameter heuristic and the well-known names); stop deferring inputs that import one of the well-known type-use annotations.

## Done when
- The fixtures of task 145 pass with their `skip` markers deleted
- `gjf::step` no longer defers inputs importing `org.jspecify.annotations.Nullable` or the other well-known type-use annotations
