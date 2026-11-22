class A {
  void f() {
    logger.atWarning().withCause(exceptionThatWasThrown).log("Some long message about the failure %s", someArgumentValue);
    logger.atInfo().log("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa %d", n);
    logger.atInfo().foo().log("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa %d", n);
  }
}
