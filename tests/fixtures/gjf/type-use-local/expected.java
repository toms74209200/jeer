import java.util.Map;
import org.checkerframework.checker.nullness.qual.NonNull;

class A {
  void f() {
    @NonNull Object a = this;
    @NonNull Map<String, Map<String, Map<String, Map<String, String>>>>
        someMuchLongerLocalVariableName = null;
  }
}
