# Add the CLI with the whitespace profile

`jeer --stdin --profile <profile>` reads Java source from stdin and writes the formatted source to stdout (design doc §6.2). Exit codes (design doc §6.3): 0 already formatted, 1 changed, 2 input rejected (syntax error; nothing on stdout, the location on stderr), 3 internal error, 4 usage error.

## Done when
- The `whitespace` profile is implemented and every `tests/fixtures/whitespace/` fixture passes with its `skip` marker deleted
- Small tests cover argument parsing and the exit code of every outcome
- An unknown profile or argument exits 4 with a usage message on stderr
