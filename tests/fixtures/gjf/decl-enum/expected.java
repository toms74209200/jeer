enum E {
  A,
  B,
  C;

  void f() {}
}

enum F {
  X(1) {
    void g() {}
  },
  Y(2);

  F(int i) {}
}
