# Format strings and modern syntax as google-java-format does

Long string reflow as Spotless's GJF step does it, text blocks, switch expressions and `case ->`, patterns and `when`, records, sealed and `permits`, module declarations, and GJF's own import ordering and unused-import removal.

## Done when
- Every `string-*`, `modern-*` and `imports-*` fixture passes with its `skip` marker deleted
- The dev corpus match rate is higher than at the start of the task
