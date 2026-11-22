# Format lambdas and method references as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitLambdaExpression` and `visitMemberReference`, with LayoutBuilder's hugging (`openHuggable`, `HugPoint`, `Choice`) that the solver ported in task 105.

## Done when
- `expr-lambda`, `expr-method-ref`, `cleanup-lambda-parens` and `cleanup-enum-constant-arguments` pass with their `skip` markers deleted
- The visitor builds `Doc::Choice`, and it is no longer covered by the `#[expect(dead_code)]` on `mod gjf`
