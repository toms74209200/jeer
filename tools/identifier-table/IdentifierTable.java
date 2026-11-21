public class IdentifierTable {
    interface Test {
        boolean of(int codePoint);
    }

    static void ranges(String name, Test test) {
        System.out.println("pub const " + name + ": &[(u32, u32)] = &[");
        int start = -1;
        for (int c = 0; c <= Character.MAX_CODE_POINT + 1; c++) {
            boolean in = c <= Character.MAX_CODE_POINT && test.of(c);
            if (in && start < 0) {
                start = c;
            } else if (!in && start >= 0) {
                System.out.printf("    (0x%X, 0x%X),%n", start, c - 1);
                start = -1;
            }
        }
        System.out.println("];");
    }

    public static void main(String[] args) {
        ranges("START", Character::isJavaIdentifierStart);
        System.out.println();
        ranges("PART", Character::isJavaIdentifierPart);
    }
}
