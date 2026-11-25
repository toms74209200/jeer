import org.jspecify.annotations.NonNull;
import org.jspecify.annotations.Nullable;
class A {
  @Override @Nullable public Object e() { return null; }
  @Override @Nullable Object f() { return null; }
  @Nullable Object g() { return null; }
  @Deprecated @NonNull String h() { return ""; }
}
