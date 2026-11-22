class A {
  void f() {
    a.b.c.d();
    this.x = y;
    super.f();
    Outer.this.x = 1;
    foo().bar().baz();
    list.stream().map(x).filter(y).collect(z);
    a[0].b[1] = c[2][3];
    (a)[0] = 1;
    x = String.class;
    x = int.class;
    x = int[].class;
    x = java.util.List.class;
    x.<T>f();
    System.out.println(String.format("%d %s", a, b));
    someObject
        .someVeryLongMethodName(argumentNumberOne, argumentNumberTwo)
        .anotherMethodCall(argumentThree)
        .yetAnotherCall();
    com.google.common.collect.ImmutableList.of(
        one, two, three, four, five, six, seven, eight, nine, ten, eleven);
    this.field.method();
    super.field.method(a);
    f(1);
    ab(c).de(f);
  }

  A() {
    <T>this(1);
  }
}
