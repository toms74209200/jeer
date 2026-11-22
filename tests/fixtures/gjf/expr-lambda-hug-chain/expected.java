class A {
  void f() {
    executor
        .submit(() -> {
          work();
        })
        .get();
    list.stream()
        .map(x -> x + 1)
        .filter(x -> x > someVeryLongThresholdValueName)
        .collect(Collectors.toList());
    builder
        .add(item -> {
          process(item);
        })
        .add(other -> {
          process(other);
        });
  }
}
