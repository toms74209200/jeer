import java.util.Map;
import org.jspecify.annotations.Nullable;

class A {
  void f(@Deprecated @Nullable Object a) {}

  void g(
      @Deprecated
          @Nullable Map<String, Map<String, Map<String, Map<String, String>>>> someLongName) {}
}
