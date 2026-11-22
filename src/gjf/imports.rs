use std::cmp::Ordering;

use super::input::{self, Tok, TokKind};
use crate::lexer;
use crate::lexer::identifier::{self, CharClass};
use crate::pipeline::{Position, Rejection};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Static,
    Module,
    Normal,
}

struct Import {
    imported: String,
    trailing: String,
    kind: Kind,
}

const IMPORT_OR_CLASS_START: [&str; 4] = ["import", "class", "interface", "enum"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Identifier,
    Space,
    Newline,
    LineComment,
    Other,
}

fn class(tok: Option<&Tok>) -> Class {
    match tok.map(|tok| {
        (
            tok.kind,
            tok.text.chars().next(),
            tok.text.starts_with("//"),
        )
    }) {
        Some((TokKind::Newline, _, _)) => Class::Newline,
        Some((TokKind::Comment(..), _, true)) => Class::LineComment,
        Some((_, Some(' ' | '\t' | '\u{c}'), _)) => Class::Space,
        Some((_, Some(c), _)) if identifier::class(c) == CharClass::Start => Class::Identifier,
        Some(_) | None => Class::Other,
    }
}

fn imported(all: &[Tok], start: usize) -> Result<(String, usize), Rejection> {
    let at = |i: usize| all.get(i).map_or("", |tok| tok.text.as_str());
    let mut i = start;
    let mut imported = String::new();
    loop {
        imported.push_str(at(i));
        i += 1;
        if at(i) != "." {
            return Ok((imported, i));
        }
        imported.push('.');
        i += 1;
        if at(i) == "*" {
            imported.push('*');
            return Ok((imported, i + 1));
        }
        if class(all.get(i)) != Class::Identifier {
            return Err(Rejection {
                position: Position::Offset(0),
                message: format!("Could not parse imported name, at: {}", at(i)),
            });
        }
    }
}

fn import(all: &[Tok], start: usize) -> Result<(Import, usize), Rejection> {
    let at = |i: usize| all.get(i).map_or("", |tok| tok.text.as_str());
    let is = |i: usize, wanted: Class| class(all.get(i)) == wanted;
    let rejection = |message: String| Rejection {
        position: Position::Offset(0),
        message,
    };
    let skip_space = |i: usize| i + usize::from(is(i, Class::Space));
    let after_keyword = skip_space(start + 1);
    let kind = match at(after_keyword) {
        "static" => Kind::Static,
        "module" => Kind::Module,
        _ => Kind::Normal,
    };
    let name = match kind {
        Kind::Normal => after_keyword,
        Kind::Static | Kind::Module => skip_space(after_keyword + 1),
    };
    if !is(name, Class::Identifier) {
        return Err(rejection(format!(
            "Unexpected token after import: {}",
            at(name)
        )));
    }
    let (imported, after_name) = imported(all, name)?;
    let semicolon = skip_space(after_name);
    if at(semicolon) != ";" {
        return Err(rejection("Expected ; after import".to_owned()));
    }
    let mut i = (semicolon..).find(|&i| at(i) != ";").unwrap_or(semicolon);
    let mut trailing = String::new();
    for wanted in [Class::Space, Class::Newline] {
        if is(i, wanted) {
            trailing.push_str(at(i));
            i += 1;
        }
    }
    while is(i, Class::LineComment) {
        trailing.push_str(at(i));
        i += 1;
        if is(i, Class::Newline) {
            trailing.push_str(at(i));
            i += 1;
        }
    }
    let end = (i..).find(|&i| at(i) != ";").unwrap_or(i);
    Ok((
        Import {
            imported,
            trailing,
            kind,
        },
        end,
    ))
}

fn scan(all: &[Tok], first: usize) -> Result<Vec<(Import, usize)>, Rejection> {
    let at = |i: usize| all.get(i).map_or("", |tok| tok.text.as_str());
    let mut scanned = Vec::new();
    let mut i = first;
    while at(i) == "import" {
        let (import, after) = import(all, i)?;
        scanned.push((import, after));
        i = (after..)
            .find(|&k| !matches!(class(all.get(k)), Class::Newline | Class::Space))
            .unwrap_or(after);
    }
    Ok(scanned)
}

pub fn reorder(text: &str) -> Result<String, Rejection> {
    let lexed = lexer::scan(text).map_err(|e| Rejection {
        position: Position::Offset(e.offset),
        message: e.kind.to_string(),
    })?;
    let all = input::toks(text, &lexed.tokens);
    let at = |i: usize| all.get(i).map_or("", |tok| tok.text.as_str());
    let find = |start: usize| {
        (start..all.len()).find(|&i| {
            class(all.get(i)) == Class::Identifier && IMPORT_OR_CLASS_START.contains(&at(i))
        })
    };
    let first = match find(0) {
        Some(first) if at(first) == "import" => first,
        Some(_) | None => return Ok(text.to_owned()),
    };
    let unindented = match first > 0 && class(all.get(first - 1)) == Class::Space {
        true => first - 1,
        false => first,
    };
    let scanned = scan(&all, first)?;
    let after_last = scanned.last().map_or(first, |(_, after)| *after);
    if find(after_last).is_some_and(|later| at(later) == "import") {
        return Err(Rejection {
            position: Position::Offset(0),
            message: "Imports not contiguous (perhaps a comment separates them?)".to_owned(),
        });
    }
    let java_order = |a: &Import, b: &Import| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.imported.encode_utf16().cmp(b.imported.encode_utf16()))
    };
    let imports = {
        let unsorted: Vec<Import> = scanned.into_iter().map(|(import, _)| import).collect();
        let sorted = {
            let mut sorted = unsorted;
            sorted.sort_by(|a, b| java_order(a, b));
            sorted
        };
        sorted
            .into_iter()
            .fold(Vec::<Import>::new(), |kept, import| {
                match kept.last().map(|last| java_order(last, &import)) {
                    Some(Ordering::Equal) => kept,
                    Some(Ordering::Less | Ordering::Greater) | None => {
                        kept.into_iter().chain([import]).collect()
                    }
                }
            })
    };
    let start = all[unindented].position;
    let prefix = &text[..start];
    let separated = match !prefix.is_empty() && !prefix.ends_with(['\n', '\r']) {
        true => "\n\n",
        false => "",
    };
    let block: String = imports
        .iter()
        .scan(
            imports.first().map(|import| import.kind),
            |previous, import| {
                let blank = match *previous == Some(import.kind) {
                    true => "",
                    false => "\n",
                };
                *previous = Some(import.kind);
                let qualifier = match import.kind {
                    Kind::Static => "static ",
                    Kind::Module => "module ",
                    Kind::Normal => "",
                };
                let ending = match import.trailing.trim_matches(|c| c <= ' ').is_empty() {
                    true => "\n",
                    false => import.trailing.as_str(),
                };
                Some(format!(
                    "{blank}import {qualifier}{};{ending}",
                    import.imported
                ))
            },
        )
        .collect();
    let after_position = all.get(after_last).map_or(text.len(), |tok| tok.position);
    let end = text[after_position..]
        .find(|c: char| !c.is_whitespace())
        .map_or(text.len(), |offset| after_position + offset);
    let closing = match end < text.len() {
        true => "\n",
        false => "",
    };
    let replacement = format!("{separated}{block}{closing}");
    match replacement == text[start..end] {
        true => Ok(text.to_owned()),
        false => Ok(format!("{}{replacement}{}", &text[..start], &text[end..])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_reorder_with_import_blocks_then_returns_them_as_import_orderer_does() {
        let cases = [
            (
                "import b.B;\nimport a.A;\n\nclass C {}\n",
                "import a.A;\nimport b.B;\n\nclass C {}\n",
            ),
            (
                "import a.A;\nimport static b.B.c;\nimport module java.base;\n\nclass C {}\n",
                "import static b.B.c;\n\nimport module java.base;\n\nimport a.A;\n\nclass C {}\n",
            ),
            (
                "import b.B; // second\n// more\nimport a.A;\n\nclass C {}\n",
                "import a.A;\nimport b.B; // second\n// more\n\nclass C {}\n",
            ),
            (
                "import a.A;\nimport a.A; // again\nimport a.*;\n\nclass C {}\n",
                "import a.*;\nimport a.A;\n\nclass C {}\n",
            ),
            (
                "package p; import b.B; import a.A;\nclass C {}\n",
                "package p;\n\nimport a.A;\nimport b.B;\n\nclass C {}\n",
            ),
            (
                "import a.A;\nimport b.B;\n\nclass C {}\n",
                "import a.A;\nimport b.B;\n\nclass C {}\n",
            ),
            ("class C {}\n", "class C {}\n"),
            ("import a.A;", "import a.A;\n"),
        ];
        for (input, expected) in cases {
            let reordered = reorder(input);

            assert_eq!(reordered, Ok(expected.to_owned()), "{input:?}");
        }
    }

    #[test]
    fn when_reorder_with_imports_it_cannot_scan_then_returns_its_message() {
        let cases = [
            ("import a.A;\nclass C {}\nimport b.B;\n", None),
            (
                "import a.A;\n/* c */\nimport b.B;\nclass C {}\n",
                Some("Imports not contiguous (perhaps a comment separates them?)"),
            ),
            ("import ;\n", Some("Unexpected token after import: ;")),
            ("import a.;\n", Some("Could not parse imported name, at: ;")),
            ("import a.A\n", Some("Expected ; after import")),
        ];
        for (input, expected) in cases {
            let message = reorder(input).err().map(|rejection| rejection.message);

            assert_eq!(message.as_deref(), expected, "{input:?}");
        }
    }
}
