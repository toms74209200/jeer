# Run the configured steps from the CLI

Bounded context: `cli` (`src/cli.rs`, `src/cli/`), calling the pipeline built from a configuration (task 171) and the file reading of task 169; `src/main.rs` passes the configuration file from `io` to `cli`.

The CLI loads the project's configuration and runs its steps; `--profile` stays for the fixtures.

## Done when
- A medium test runs jeer on a temporary project with a configuration and gets the oracle's output for the same Spotless setup
- Small tests cover choosing between a configuration and `--profile`, and the error when neither is given
