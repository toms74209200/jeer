import java.lang.reflect.Field;
import java.util.Map;
import java.util.TreeMap;

public class EntityTable {
    public static void main(String[] args) throws ReflectiveOperationException {
        Class<?> entities = Class.forName("org.commonmark.internal.util.Html5Entities");
        Field field = entities.getDeclaredField("NAMED_CHARACTER_REFERENCES");
        field.setAccessible(true);
        @SuppressWarnings("unchecked")
        Map<String, String> named = new TreeMap<>((Map<String, String>) field.get(null));
        System.out.println("pub const NAMED_CHARACTER_REFERENCES: &[(&str, &str)] = &[");
        for (Map.Entry<String, String> entry : named.entrySet()) {
            StringBuilder value = new StringBuilder();
            entry.getValue().codePoints().forEach(c -> value.append(String.format("\\u{%X}", c)));
            System.out.printf("    (\"%s\", \"%s\"),%n", entry.getKey(), value);
        }
        System.out.println("];");
    }
}
