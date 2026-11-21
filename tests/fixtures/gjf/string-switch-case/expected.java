class A {
  void f(String s) {
    switch (s) {
      case "a" -> g();
      case "b", "c" -> h();
      default -> {}
    }
  }
}
