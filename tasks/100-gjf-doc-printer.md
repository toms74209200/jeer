# Add the Doc IR and printer of google-java-format

Port GJF's `Doc` model (levels, breaks with fill modes, tokens, spaces) and its width-fitting printer, preserving semantics (design doc §9). The `fits` check stays near-linear.

## Done when
- Small tests reproduce GJF's breaking decisions for nested levels, `UNIFIED`/`INDEPENDENT`/`FORCED` breaks and optional breaks, on hand-built docs
- A benchmark-style small test on a doc of 100k nodes runs in linear time
