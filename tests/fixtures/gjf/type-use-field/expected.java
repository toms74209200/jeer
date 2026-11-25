import java.util.Map;
import org.jspecify.annotations.Nullable;

class A {
  @Deprecated @Nullable Object short1;

  @Deprecated
  @Nullable Map<String, Map<String, Map<String, Map<String, String>>>> someLongFieldName;
}
