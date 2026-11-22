@A({1,2}) @B(x={1,2}, y=2) @C(x=1, y="s") @java.lang.Deprecated @D(@E) @F(x=@G) class A {
  @A(1) int x;
  @A int y;
  void f(@A int a, final int b, @B(1) int c) { @A int z = 1; @A @B int w = 2; @A(1) int v = 3; }
}
