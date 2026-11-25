import org.jspecify.annotations.Nullable;
class A {
  @Override public <T> @Nullable T f() { return null; }
  @Deprecated <T> @Deprecated T g() { return null; }
}
