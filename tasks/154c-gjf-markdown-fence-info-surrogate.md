# Carry lone surrogates through markdown Javadoc's rewrite

Bounded context: `gjf` (`src/gjf/javadoc/`, `src/gjf.rs`), and the text `pipeline` passes between runs of a step.

A fence info's numeric entity can decode to a lone UTF-16 surrogate (`&#xDE00;`, `&#55357;`). GJF decodes the info once per run, but Spotless runs the step again until its output stops changing (jeer's `pipeline::padded_cell`), so `&amp;amp;amp;amp;amp;amp;` ends as `&`. Java keeps a lone surrogate in the String between runs: one left by a run can pair with one a later run decodes (`a&#xD83D;&amp;#xDE00;b`, `a&amp;#xD83D;&#xDE00;b` and `a&#55357;&amp;#56832;b` all become `a😀b`), and one still unpaired at the end is written as `?` by Java's UTF-8 encoder (`a&#xDE00;b`, `a&amp;#xDE00;b` and `a&amp;amp;#xDE00;b` become `a?b`). jeer's text is a `String` in the step and in `pipeline` between runs, which cannot hold one, so task 154b defers a fence whose info leaves an unpaired surrogate on any run. Carrying one needs a representation other than `String` in both contexts, so this task may split into one per context.

A surrogate that a run decodes only after the line has stopped being a fence (`a&#96;&amp;#xDE00;b`, where the second run reads a backtick fence's info holding a backtick as a paragraph) is paragraph text, which GJF formats; task 154b defers it too, since it cannot tell the runs apart.

## Done when
- `parse` no longer defers a fence info that leaves an unpaired surrogate
- Oracle fixtures, added in a fixtures task first, cover a lone surrogate and a pair split across runs, and pass
