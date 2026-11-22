# Write the step's fixed point as Spotless does

Bounded context: `pipeline` (`src/pipeline.rs`, `src/pipeline/`): a wrapper from a step to a step, composed around the gjf step in its profile.

Spotless's PaddedCell applies a step again until its output stops changing, and writes that fixed point. google-java-format is not idempotent for some inputs: a trailing `//` comment that JavaCommentsHelper wraps moves the declaration's breaks on the next pass (`int a; // ...long...` comes out as `int\n      a; // ...` after one pass and as `int a; // ...\n  // ...` after two).

## Done when
- The fixtures of task 117-fixtures-padded-cell pass with their `skip` markers deleted
- The step is wrapped by a function that applies it to its fixed point as PaddedCell does (composition policy), including what PaddedCell does when the output cycles
