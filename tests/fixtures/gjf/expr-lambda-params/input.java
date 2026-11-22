class A {
  void f() {
    Comparator<String> c = Comparator.<String, Integer>comparing(String::length);
    consume((String... parts) -> parts.length);
    Function<Integer, int[]> g = int[]::new;
    Function<List<String>, Integer> h = List::<String>size;
  }
}
