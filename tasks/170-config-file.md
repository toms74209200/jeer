# Add the configuration file

Bounded context: a new `config` context (`src/config.rs`, `src/config/`).

Decide design doc §6.4 (interpret the Spotless block, or a config file of our own plus a migration path), record the decision with `shampoo ask`, and implement the parse: a configuration selects the Phase 1 steps and their order as Spotless does. The CLI uses it in task 171.

## Done when
- The design doc's §6.4 and its "To decide" item are updated with the decision
- The configuration parses into a type that lists the selected steps in order, and small tests cover parsing it and every error in it
