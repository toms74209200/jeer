class A {
  void f() {
    outer:
    for (; ; ) {
      for (; ; ) {
        break outer;
      }
    }
  }
}
