class A {
  void f() {
    synchronized (lockObjectWithAVeryLongName.getAnotherLockWithLongName().andAThirdOneWithALongName()) {
      a();
    }
  }
}
