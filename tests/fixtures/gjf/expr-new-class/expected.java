class A {
  void f(Outer outer) {
    Object a = new StringBuilder("x");
    List<String> b = new ArrayList<>();
    Map<String, Integer> c = new HashMap<String, Integer>(16);
    Object d = new <String>Generic("y");
    Outer.Inner e = outer.new Inner();
    Runnable r =
        new Runnable() {
          public void run() {
            g();
          }
        };
    new Thread(
            () -> {
              work();
            })
        .start();
    Thread t =
        new Thread(
            () -> {
              work();
            }) {
          public void interrupt() {}
        };
  }
}
