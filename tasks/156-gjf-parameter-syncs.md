# Sync catch and lambda parameters as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

Two sync calls of JavaInputAstVisitor are not mapped in jeer. A catch parameter is synced by GJF in both forms: `scan(ex)` reaches visitVariable's `sync(node)`, and the union branch runs visitUnionType's `sync(declaration)`. jeer's catch_clause never syncs it. A lambda parameter goes through visitVariables and declareOne, neither of which syncs, but jeer's lambda syncs each one.

## Done when
- Visitor::catch_clause syncs the catch parameter where GJF does, in both the single and the union form
- The lambda no longer syncs its parameters
- Small tests show the sync positions through the rejection offsets they set (an oracle-rejected input inside a catch parameter and inside a lambda parameter, expected messages from the oracle)
