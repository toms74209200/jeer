class A {
  void f() {
    try {
      a();
    } catch (IOException | RuntimeException e) {
      b();
    } finally {
      c();
    }
    try (var r = open();
        var s = open()) {
      d(r);
    }
  }
}
