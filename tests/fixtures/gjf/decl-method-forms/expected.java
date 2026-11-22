abstract class A {
  <T> T f(T t) {
    return t;
  }

  <T> A(T t) {}

  int[] g()[] {
    return null;
  }

  void h() throws java.io.IOException, RuntimeException {}

  void i(String... a) {}

  void j(int a[], int[]... b) {}

  void k() {
    return;
  }

  public static synchronized <K, V> java.util.Map<K, V> longMethodNameThatIsLong(
      K firstArgument, V secondArgument, java.util.function.Function<K, V> f) throws Exception {
    return null;
  }

  static {
  }

  {
    int x = 1;
  }

  void l() {
    {
      int y;
    }
  }
}
