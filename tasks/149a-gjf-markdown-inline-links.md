# Port inline links for markdown Javadoc

Bounded context: `gjf` (`src/gjf/javadoc/`).

commonmark-java's inline parser reads an inline link's destination and title (`[text](dest "title")`, InlineParserImpl's close-bracket handling with the bracket stack and LinkScanner), so a backtick inside them opens no code span. Task 149 ports code spans but defers a paragraph or heading where a backtick follows a `](`, since jeer does not find links yet.

## Done when
- `parse` no longer defers a backtick after `](`, and finds the code spans commonmark-java finds around inline links and images
- Small tests check inline links with backticks in the text, the destination and the title against the oracle
