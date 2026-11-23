class A {
  void f() {
    String s = new Object() { public String toString() { return "a"; } }.toString();
    int n = new Object() { int size() { return 1; } }.toString().trim().length();
  }
}
