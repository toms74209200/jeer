public class LetterTable {
    public static void main(String[] args) {
        System.out.println("pub const LETTER_OR_DIGIT: &[(u32, u32)] = &[");
        int start = -1;
        for (int c = 0; c <= Character.MAX_VALUE + 1; c++) {
            boolean in = c <= Character.MAX_VALUE && Character.isLetterOrDigit((char) c);
            if (in && start < 0) {
                start = c;
            } else if (!in && start >= 0) {
                System.out.printf("    (0x%X, 0x%X),%n", start, c - 1);
                start = -1;
            }
        }
        System.out.println("];");
    }
}
