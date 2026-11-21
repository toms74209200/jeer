class A {
  void f() {
    outer(
        middle(
            inner(deepest(argumentNumberOne, argumentNumberTwo), anotherArgument),
            yetAnotherArgument),
        lastArgument);
  }
}
