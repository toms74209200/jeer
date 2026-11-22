class A {
  void f(Exception e) {
    assert x;
    assert veryLongConditionNumberOne(argumentNumberOne) && veryLongConditionNumberTwo(argumentNumberTwo) : "a detail message";
    throw e;
  }
}
