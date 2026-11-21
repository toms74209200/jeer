class A {
  void f() {
    Supplier<List<String>> s = ArrayList::new;
    Function<String, Integer> l = String::length;
    g(this::h, super::i);
  }
}
