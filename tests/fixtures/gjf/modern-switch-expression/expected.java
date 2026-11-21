class A {
  int f(Object o) {
    return switch (o) {
      case Integer i when i > 0 -> i;
      case Integer i -> -i;
      case String s -> s.length();
      default -> 0;
    };
  }
}
