enum A {}

enum B {
  X;
}

enum C {
  X,
  Y,
}

enum D implements I, J {
  X
}

enum E {
  @Deprecated
  X,
  Y(),
  Z(1, 2)
}

enum F {
  ;

  void f() {}
}

enum G {
  X {
    int y;
  };
  int z;
}
