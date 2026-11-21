class A {
  void f() {
    for (int i = 0; i < n; i++) {
      g(i);
    }
    for (String s : list) h(s);
    while (x) {
      x = f();
    }
    do {
      y();
    } while (z);
  }
}
