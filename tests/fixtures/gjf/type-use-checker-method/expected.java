import org.checkerframework.checker.nullness.qual.NonNull;
import org.checkerframework.checker.nullness.qual.Nullable;

class A {
  @Override
  @Nullable
  public Object e() {
    return null;
  }

  @Override
  @Nullable Object f() {
    return null;
  }

  @Nullable Object g() {
    return null;
  }

  @Deprecated
  @NonNull String h() {
    return "";
  }
}
