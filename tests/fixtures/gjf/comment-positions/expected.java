class A {
  void f() {
    g(/* x= */ 1, /* y= */ 2);
    g(/* c */ 1);
    int a = 1; /* trailing */
    int b = 2; // trailing line
    /* before */ int c = 3;
    // line one

    // line two
    int d = 4;
  }

  /* block
  comment */
  int x;
}
