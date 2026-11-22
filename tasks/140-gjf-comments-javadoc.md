# Format comments and Javadoc as google-java-format does

Comment placement and re-indentation of line and block comments, and GJF's Javadoc formatter (wrapping, `<p>`, `<pre>`, `{@code}`, block tags).

## Done when
- Every `comment-*` and `javadoc-*` fixture passes with its `skip` marker deleted
- The dev corpus match rate is higher than at the start of the task
- `gjf::format` no longer defers inputs with Javadoc or `///` comments (GJF treats a `///` comment at a declaration as markdown Javadoc whatever its length), and LayoutBuilder's Javadoc branches are ported with it: the forced break after a Javadoc before a token, no blank line requested after it, and a trailing Javadoc broken onto its own line
