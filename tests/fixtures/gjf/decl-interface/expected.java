interface I extends J, K {
  int X = 1;

  void f();

  default void g() {}
}
