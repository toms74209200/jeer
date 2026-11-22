# Add the configuration file

Bounded context: a new `config` context (`src/config.rs`, `src/config/`), which the CLI only calls.

Decide design doc §6.4 (interpret the Spotless block, or a config file of our own plus a migration path), record the decision with `shampoo ask`, and implement it: a project selects the Phase 1 steps and their order as Spotless does, and `--profile` stays for the fixtures.

## Done when
- The design doc's §6.4 and its "To decide" item are updated with the decision
- Small tests cover parsing the configuration and every error in it
- A medium test runs jeer on a temporary project with a configuration and gets the oracle's output for the same Spotless setup
