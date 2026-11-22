class A {
  void f() {
    run(() -> {
      a();
      b();
    });
    callWithManyArguments(
        first,
        (a, b) -> {
          combine(a, b);
        });
    foo.bar(
        x -> doSomethingWithAVeryLongMethodName(x, anotherArgumentThatIsLong, yetAnotherArgument));
    submit( // why
        () -> {
          work();
        });
  }
}
