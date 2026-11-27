pub fn classic_comment_text(input: &str) -> String {
    let stripped = input[3..input.len() - 2].trim_end_matches('*');
    let lines: Vec<&str> = stripped.lines().collect();
    let (first, continuations) = lines.split_first().unwrap_or((&"", &[]));
    let star = |line: &str| {
        let spaces = line.len() - line.trim_start_matches([' ', '\t']).len();
        line[spaces..]
            .strip_prefix('*')
            .map(|after| spaces + 1 + usize::from(after.starts_with([' ', '\t'])))
    };
    let star_prefix = continuations
        .iter()
        .filter_map(|line| star(line))
        .min()
        .unwrap_or(0);
    let non_star_leading = continuations
        .iter()
        .filter(|line| star(line).is_none())
        .filter_map(|line| line.find(|c| c != ' ' && c != '\t'))
        .min()
        .unwrap_or(0);
    let strip = star_prefix.min(non_star_leading);
    let processed: Vec<&str> = std::iter::once(*first)
        .chain(continuations.iter().map(|line| match star(line) {
            Some(prefix) => &line[prefix..],
            None => line.get(strip..).unwrap_or(""),
        }))
        .collect();
    strip_common_leading_whitespace(&processed)
}

pub fn markdown_comment_text(input: &str) -> String {
    let lines: Vec<&str> = input
        .lines()
        .map(|line| line.find("///").map_or("", |start| &line[start + 3..]))
        .collect();
    strip_common_leading_whitespace(&lines)
}

fn strip_common_leading_whitespace(lines: &[&str]) -> String {
    let leading = lines
        .iter()
        .filter_map(|line| line.find(|c| c != ' ' && c != '\t'))
        .min()
        .unwrap_or(0);
    let stripped: Vec<&str> = lines
        .iter()
        .map(|line| line.get(leading..).unwrap_or(""))
        .collect();
    stripped.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_classic_comment_text_with_each_shape_then_returns_gjf_comment_text() {
        let cases = [
            ("/** One line. */", "One line. "),
            (
                "/**\n * First.\n *\n *   indented\n */",
                "\nFirst.\n\n  indented\n ",
            ),
            ("/**\n   no star\n   second\n */", "\nno star\nsecond\n"),
            ("/**\n * a\n  b\n */", "\na\nb\n"),
            ("/** text ***/", "text "),
            ("/** */", " "),
        ];
        for (input, expected) in cases {
            let text = classic_comment_text(input);

            assert_eq!(text, expected, "{input:?}");
        }
    }

    #[test]
    fn when_markdown_comment_text_with_each_shape_then_returns_gjf_comment_text() {
        let cases = [
            ("/// One line.", "One line."),
            (
                "/// First.\n  ///\n  ///   indented",
                "First.\n\n  indented",
            ),
            ("///a\n    ///  b", "a\n  b"),
            ("///   a\n///   b", "a\nb"),
        ];
        for (input, expected) in cases {
            let text = markdown_comment_text(input);

            assert_eq!(text, expected, "{input:?}");
        }
    }
}
