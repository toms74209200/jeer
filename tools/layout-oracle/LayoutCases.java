import com.google.common.collect.ImmutableList;
import com.google.googlejavaformat.Indent;
import com.google.googlejavaformat.Output.BreakTag;
import com.google.googlejavaformat.layout.Doc;
import com.google.googlejavaformat.layout.LayoutSolver;
import java.util.Optional;

public class LayoutCases {
    static Doc text(String s) {
        return new Doc.Text(s);
    }

    static Doc.Break brk(Doc.BreakMode mode, String flat, int indent) {
        return new Doc.Break(mode, flat, Indent.Const.make(indent, 1), Optional.empty());
    }

    static Doc.Group group(int indent, Doc... docs) {
        return new Doc.Group(Indent.Const.make(indent, 1), ImmutableList.copyOf(docs));
    }

    static void show(String name, Doc doc, int width) {
        String out = LayoutSolver.format(doc, width);
        System.out.println(name + "\t" + out.replace("\\", "\\\\").replace("\n", "\\n"));
    }

    public static void main(String[] args) {
        for (Doc.BreakMode mode : Doc.BreakMode.values()) {
            for (int width : new int[] {20, 8}) {
                show("modes/" + mode + "/" + width,
                        group(0, text("aaa"), brk(mode, " ", 4), text("bbb"), brk(mode, " ", 4), text("ccc")),
                        width);
            }
        }
        show("previous-too-wide",
                group(0, text("a"), brk(Doc.BreakMode.FILL, " ", 2), text("bbbbbbbbbbbb"), brk(Doc.BreakMode.FILL, " ", 2), text("c")),
                10);
        show("nested",
                group(2, text("return"), brk(Doc.BreakMode.UNIFIED, " ", 4),
                        group(0, text("f("), brk(Doc.BreakMode.UNIFIED, "", 4), text("x)")),
                        Doc.Space.INSTANCE, text("+ y;")),
                12);
        for (int width : new int[] {30, 12}) {
            BreakTag tag = new BreakTag();
            show("tagged/" + width,
                    group(0, text("first"), new Doc.Break(Doc.BreakMode.UNIFIED, " ", Indent.Const.make(4, 1), Optional.of(tag)),
                            text("second"),
                            new Doc.Break(Doc.BreakMode.UNIFIED, " ", Indent.If.make(tag, Indent.Const.make(8, 1), Indent.Const.make(2, 1)), Optional.empty()),
                            text("third")),
                    width);
        }
        for (int width : new int[] {40, 20, 12}) {
            Doc.Group body = group(0, text("{"), brk(Doc.BreakMode.FORCED, "", 2), text("work();"), brk(Doc.BreakMode.FORCED, "", 0), text("}"));
            Doc.Group preferred = group(0, text("call(x ->"), Doc.Space.INSTANCE, body, text(")"));
            Doc.Group fallback = group(0, text("call("), brk(Doc.BreakMode.UNIFIED, "", 4), group(0, text("x ->"), Doc.Space.INSTANCE, body), text(")"));
            show("choice/" + width, new Doc.Choice(preferred, 9, fallback, false), width);
        }
        for (int width : new int[] {40, 16}) {
            Doc.Group preferred = group(0, text("f(x ->"), brk(Doc.BreakMode.UNIFIED, " ", 4), text("g(x))"));
            Doc.Group fallback = group(0, text("f("), brk(Doc.BreakMode.UNIFIED, "", 4), text("x -> g(x))"));
            show("choice-next-line/" + width, new Doc.Choice(preferred, 6, fallback, true), width);
        }
        show("choice-next-line-fallback",
                new Doc.Choice(
                        group(0, text("callSomething(x ->"), brk(Doc.BreakMode.UNIFIED, " ", 4), text("gg(xx))")),
                        18,
                        group(0, text("callSomething("), brk(Doc.BreakMode.UNIFIED, "", 4), text("x -> gg(xx))")),
                        true),
                20);
        for (int width : new int[] {24, 14}) {
            BreakTag trialTag = new BreakTag();
            Doc.Group inner = group(0, text("{"), brk(Doc.BreakMode.FORCED, "", 2), text("b();"), brk(Doc.BreakMode.FORCED, "", 0), text("}"));
            Doc.Group innerPreferred = group(0, text("g(y ->"), Doc.Space.INSTANCE, inner, text(")"));
            Doc.Group innerFallback = group(0, text("g("), brk(Doc.BreakMode.UNIFIED, "", 4), group(0, text("y ->"), Doc.Space.INSTANCE, inner), text(")"));
            Doc.Group outerBody = group(0, text("{"),
                    new Doc.Break(Doc.BreakMode.UNIFIED, "", Indent.Const.make(2, 1), Optional.of(trialTag)),
                    new Doc.Choice(innerPreferred, 6, innerFallback, false),
                    new Doc.Break(Doc.BreakMode.UNIFIED, "", Indent.If.make(trialTag, Indent.Const.make(0, 1), Indent.Const.make(6, 1)), Optional.empty()),
                    text("}"));
            Doc.Group outerPreferred = group(0, text("f(x ->"), Doc.Space.INSTANCE, outerBody, text(")"));
            Doc.Group outerFallback = group(0, text("f("), brk(Doc.BreakMode.UNIFIED, "", 4), group(0, text("x ->"), Doc.Space.INSTANCE, outerBody), text(")"));
            show("nested-choice/" + width, new Doc.Choice(outerPreferred, 6, outerFallback, false), width);
        }
        Doc.Group hugged = group(0, text("{"), brk(Doc.BreakMode.FORCED, "", 2), text("a();"), brk(Doc.BreakMode.FORCED, "", 0), text("}"));
        Doc.Group preferredHeader = group(0, text("run(() ->"), Doc.Space.INSTANCE, hugged, text(")"));
        Doc.Group fallbackHeader = group(0, text("run("), brk(Doc.BreakMode.UNIFIED, "", 4), group(0, text("() ->"), Doc.Space.INSTANCE, hugged), text(")"));
        show("header",
                group(0, Doc.HeaderMark.START, text("if ("), new Doc.Choice(preferredHeader, 9, fallbackHeader, false), text(")"), Doc.HeaderMark.END, text(" {}")),
                30);
        show("lines",
                group(2, text("s ="), brk(Doc.BreakMode.FORCED, " ", 4),
                        new Doc.Lines(ImmutableList.of("\"\"\"", "  a", "", "b", "\"\"\";"), Doc.EMPTY_RANGE)),
                20);
        show("multiline-text", group(0, text("x"), Doc.Space.INSTANCE, text("/* a\n b */"), Doc.Space.INSTANCE, text("y")), 40);
    }
}
