class A {
  boolean f(Object o) {
    if (o instanceof String) {}
    boolean b = !(o instanceof Number);
    return someVeryLongReceiverExpressionName.getSomethingWithAVeryLongName() instanceof java.util.RandomAccess;
  }
}
