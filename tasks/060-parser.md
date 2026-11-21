# Add a lossless CST parser for Java 25

Hand-written recursive descent producing a lossless CST (design doc §8, option B): contextual keywords (`var`, `record`, `sealed`, `permits`, `yield`, `when`), lambda vs. cast, `>>` vs. nested generics, every Java 25 construct including modules, records, sealed types, patterns, switch expressions and text blocks.

## Done when
- Small tests cover each construct and the ambiguities above
- `scripts/check-roundtrip.sh dev` also checks that every dev corpus file parses without error and its CST reproduces the input byte for byte
- The `gjf` profile rejects input with exit 2 exactly where the oracle does: the fixtures from 050 pass with their `skip` markers deleted (other output may stay unformatted until later tasks)
- Parsing time is near-linear in input size: a small test or benchmark on generated deeply nested input and long method chains shows no superlinear growth
