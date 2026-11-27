# Lex HTML in markdown Javadoc

Bounded context: `gjf` (`src/gjf/javadoc/`).

GJF's lexer recognizes the HTML tags of classic Javadoc in markdown Javadoc too, and the writer handles some of them differently there (`writeListOpen` indents by 2 only for a non-empty tag and requests blank lines around lists only in classic Javadoc). An HTML block (HtmlBlockParser's seven start conditions) is a block of its own, not a paragraph, so it also decides where the paragraph positions are. A link reference definition's destination in `<...>` (LinkScanner.scanLinkDestination's angle-bracket branch) is ported here too, since the `<` deferral hides it until then. Task 147 also defers a markdown comment holding U+0000, which GJF formats: commonmark-java replaces it with U+FFFD when it reads a line (DocumentParser.prepareLine) and keeps every position, and GJF's lexer reads the original text. Task 152 defers a fenced code block whose info string holds `&`, since commonmark-java unescapes the info with Escaping.unescapeString, which also decodes HTML entities (Html5Entities). Task 147 defers every `<` in markdown Javadoc. The deferrals where GJF rejects the file, reported at `LINE_UNDEFINED`, are task 157b's.

## Done when
- `parse` no longer defers a `<`, a U+0000 or an `&` in a fence's info string
- Oracle fixtures, added in a fixtures task first, cover HTML tags in markdown Javadoc, and pass
