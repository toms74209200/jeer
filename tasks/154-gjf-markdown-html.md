# Lex HTML in markdown Javadoc

Bounded context: `gjf` (`src/gjf/javadoc/`).

GJF's lexer recognizes the HTML tags of classic Javadoc in markdown Javadoc too, and the writer handles some of them differently there (`writeListOpen` indents by 2 only for a non-empty tag and requests blank lines around lists only in classic Javadoc). An HTML block (HtmlBlockParser's seven start conditions) is a block of its own, not a paragraph, so it also decides where the paragraph positions are. A link reference definition's destination in `<...>` (LinkScanner.scanLinkDestination's angle-bracket branch) is ported here too, since the `<` deferral hides it until then. Task 147 defers every `<` in markdown Javadoc; this is the last construct it defers.

## Done when
- `parse` defers nothing: it returns `Option` no more, and `gjf::step` no longer defers an input because of a `///` comment
- Oracle fixtures, added in a fixtures task first, cover HTML tags in markdown Javadoc, and pass
