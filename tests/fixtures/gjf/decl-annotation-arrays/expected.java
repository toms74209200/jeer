class A {
  @A({
    {
      "aaaaaaaaaaaa", "bbbbbbbbbbbb",
    }
  })
  int d;

  @A(
      x = {
        {
          "aaaaaaaaaaaa", "bbbbbbbbbbbb",
        }
      })
  int e;

  @A(
      x = {
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "cccccccccccccccc"
      })
  int f;
}
