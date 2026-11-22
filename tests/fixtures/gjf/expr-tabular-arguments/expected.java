class A {
  void f() {
    g(
        1, 2,
        3, 4);
    h(
        "a", 1,
        "b", 2,
        "c", 3);
  }

  int[] a = {
    1, 2,
    3, 4,
  };
  int[][] b = {
    {1, 2},
    {3, 4},
  };
  String[] c = {
    "aaaaaaaaaaaaaaaa",
    "bbbbbbbbbbbbbbbbbbbbbbb",
    "ccccccccccccccccccccccccc",
    "ddddddddddddddddddddddddddddd",
    "eeee"
  };

  @A({
    "aaaaaaaaaaaaaaaaaaaaaaaa",
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "cccccccccccccccccccccccccccccccc"
  })
  int d;

  @A(
      x = {
        "aaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "cccccccccccccccccccccccccccccc"
      })
  int e;
}
