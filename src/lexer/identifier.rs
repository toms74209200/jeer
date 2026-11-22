use super::identifier_table;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharClass {
    Start,
    Part,
    Other,
}

pub fn class(c: char) -> CharClass {
    let within = |table: &[(u32, u32)]| {
        table
            .binary_search_by(
                |&(low, high)| match (high < u32::from(c), low > u32::from(c)) {
                    (true, _) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    (false, false) => std::cmp::Ordering::Equal,
                },
            )
            .is_ok()
    };
    match (
        within(identifier_table::START),
        within(identifier_table::PART),
    ) {
        (true, _) => CharClass::Start,
        (false, true) => CharClass::Part,
        (false, false) => CharClass::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_class_with_characters_then_returns_their_java_identifier_class() {
        let cases = [
            ('a', CharClass::Start),
            ('$', CharClass::Start),
            ('_', CharClass::Start),
            ('7', CharClass::Part),
            ('\u{0}', CharClass::Part),
            (' ', CharClass::Other),
            ('(', CharClass::Other),
        ];
        for (c, expected) in cases {
            let classified = class(c);

            assert_eq!(classified, expected, "{c:?}");
        }
    }
}
