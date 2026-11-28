# Report javac's message for an enum constant without a name

Bounded context: `parser` (`src/parser/`).

At an enum constant without annotations javac does not reach ident() for a missing name, and reports another error at the next token. Task 155 keeps jeer at that line, but jeer's message is still `<identifier> expected`. The lambda and case-label sites are task 155h's. Found in task 155's review, oracle against jeer:
- An enum constant without modifiers (`enum E {` then `1 }`): `L2 ... enum constant expected here`

## Done when
- Oracle fixtures, added in a fixtures task first, cover it, and pass with javac's message
- The parser reports it with javac's message at javac's line
