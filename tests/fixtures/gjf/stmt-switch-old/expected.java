class A {
  void f(int x) {
    switch (x) {
      case 1:
      case 2:
        a();
        break;
      default:
        b();
    }
  }
}
