class A {
  void f() {
    list.stream()
        .filter(x -> x > 0)
        .map(x -> x * 2)
        .sorted()
        .collect(java.util.stream.Collectors.toList())
        .forEach(System.out::println);
  }
}
