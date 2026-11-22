enum E {
  X(
      () -> {
        f();
      }) {
    void g() {}
  };
}
