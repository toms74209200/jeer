# Decode HTML entities in a markdown fence's info string

Bounded context: `gjf` (`src/gjf/javadoc/`).

Task 152 defers a fenced code block whose info string holds `&`, since commonmark-java unescapes the info with Escaping.unescapeString, which also decodes HTML entities: named ones from Html5Entities' table (`entities.txt` in commonmark-java 0.29.0), and decimal and hexadecimal ones through Html5Entities.entityToString. GJF writes the decoded info after the opening fence (JavadocWriter.writeMarkdownFencedCodeBlock). Task 154 ported the `<` deferral and split this one off.

## Done when
- `parse` no longer defers an `&` in a fence's info string
- The fixture `javadoc-markdown-fence-info-entity` passes
