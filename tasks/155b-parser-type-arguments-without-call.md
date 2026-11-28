# Report type arguments without a call as javac does

Bounded context: `parser` (`src/parser/expressions.rs`).

Explicit type arguments at the start of an expression must be followed by `this` or `super` (an explicit constructor invocation) for jeer's parser, which otherwise reports `<identifier> expected` at the next token (`expressions.rs`, the `Punct::Lt` arm). javac reports `illegal start of expression` there, and Spotless prints that message. Found in task 155:

```java
class A {
  void f() {
    <T>
    ;
  }
}
```

The oracle rejects it at `L4 ... illegal start of expression`, and jeer at 4:5 with `<identifier> expected`; the same holds for `<T>` then `x();` in a constructor.

## Done when
- Oracle fixtures, added in a fixtures task first, cover type arguments followed by something other than `this` or `super`, and pass with javac's message
- The parser reports javac's message at javac's position for them
