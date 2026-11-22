use super::doc::parameter_comment;

pub fn rewrite<'a>(text: &'a str, max_width: usize, column: usize) -> String {
    let indent = " ".repeat(column);
    match (text.starts_with("//"), text.starts_with("/*")) {
        (false, false) => text.to_owned(),
        (true, _) => {
            let mut wrapped: Vec<String> = Vec::new();
            for line in text
                .split('\n')
                .map(|line| line.trim_matches(char::is_whitespace))
            {
                let slashes = line.chars().take_while(|&c| c == '/').count();
                let missing_space = line[slashes..]
                    .chars()
                    .next()
                    .is_some_and(|c| !matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r'));
                let rest = &line[slashes..];
                let exempt = rest.starts_with("noinspection")
                    || rest
                        .strip_prefix("$NON-NLS-")
                        .and_then(|r| r.split_once('$'))
                        .is_some_and(|(digits, _)| {
                            !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
                        });
                let mut line: Vec<u16> = match missing_space && !exempt {
                    true => format!("{} {rest}", &line[..slashes])
                        .encode_utf16()
                        .collect(),
                    false => line.encode_utf16().collect(),
                };
                if String::from_utf16_lossy(&line).starts_with("// MOE:") {
                    wrapped.push(String::from_utf16_lossy(&line));
                    continue;
                }
                while line.len() + column > max_width {
                    let mut index = max_width.saturating_sub(column);
                    while index >= 2
                        && !char::from_u32(u32::from(line[index])).is_some_and(char::is_whitespace)
                    {
                        index -= 1;
                    }
                    if index <= 2 {
                        break;
                    }
                    wrapped.push(String::from_utf16_lossy(&line[..index]));
                    line = "//"
                        .encode_utf16()
                        .chain(line[index..].iter().copied())
                        .collect();
                }
                wrapped.push(String::from_utf16_lossy(&line));
            }
            let trimmed: Vec<&str> = wrapped
                .iter()
                .map(|line| line.trim_matches(|c: char| c <= ' '))
                .collect();
            trimmed.join(&format!("\n{indent}"))
        }
        (false, true) => {
            let java_trim = |line: &'a str| -> &'a str { line.trim_matches(|c: char| c <= ' ') };
            let lines: Vec<&str> = text
                .split('\n')
                .map(|line| line.trim_end_matches(char::is_whitespace))
                .collect();
            let javadoc_shaped = match lines.first().map(|first| java_trim(first)) {
                Some(first) if first.starts_with("/**") => true,
                Some(first) if first.starts_with("/*") => lines[1..]
                    .iter()
                    .all(|line| java_trim(line).starts_with('*')),
                Some(_) | None => false,
            };
            match (parameter_comment(text), javadoc_shaped) {
                (Some(reformatted), _) => reformatted,
                (None, true) => {
                    let inner = " ".repeat(column + 1);
                    let mut out = java_trim(lines[0]).to_owned();
                    for line in &lines[1..] {
                        let line = java_trim(line);
                        out.push('\n');
                        out.push_str(&inner);
                        if !line.starts_with('*') {
                            out.push_str("* ");
                        }
                        out.push_str(line);
                    }
                    out
                }
                (None, false) => {
                    let start = lines[1..]
                        .iter()
                        .filter_map(|line| line.chars().position(|c| !c.is_whitespace()))
                        .min()
                        .unwrap_or(0);
                    let mut out = lines[0].to_owned();
                    for line in &lines[1..] {
                        out.push('\n');
                        out.push_str(&indent);
                        match line.char_indices().nth(start) {
                            Some((offset, _)) => out.push_str(&line[offset..]),
                            None if line.chars().count() == start => {}
                            None => out.push_str(line),
                        }
                    }
                    out
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_rewrite_with_comments_then_returns_them_as_java_comments_helper_does() {
        let cases = [
            ("x", 100, 4, "x"),
            ("//no space", 100, 0, "// no space"),
            ("///triple", 100, 0, "/// triple"),
            ("//noinspection Foo", 100, 0, "//noinspection Foo"),
            ("//$NON-NLS-1$", 100, 0, "//$NON-NLS-1$"),
            ("// MOE: a b c d e f", 10, 0, "// MOE: a b c d e f"),
            ("// aaa bbb ccc", 10, 0, "// aaa bbb\n// ccc"),
            ("// aaa bbb ccc", 10, 2, "// aaa\n  // bbb\n  // ccc"),
            ("// aaaaaaaaaaaa", 10, 0, "// aaaaaaaaaaaa"),
            ("/* a */", 100, 4, "/* a */"),
            ("/*\n * a\n */", 100, 2, "/*\n   * a\n   */"),
            ("/**\n   a\n */", 100, 0, "/**\n * a\n */"),
            (
                "/*\n      a\n        b\n   */",
                100,
                2,
                "/*\n     a\n       b\n  */",
            ),
            ("/*\n    a\n\n    b */", 100, 0, "/*\na\n\nb */"),
            ("/*x=*/", 100, 0, "/* x= */"),
            ("/*\n\u{1f}* a\n   */", 100, 2, "/*\n   * a\n   */"),
        ];
        for (text, max_width, column, expected) in cases {
            let rewritten = rewrite(text, max_width, column);

            assert_eq!(rewritten, expected, "{text:?} {max_width} {column}");
        }
    }
}
