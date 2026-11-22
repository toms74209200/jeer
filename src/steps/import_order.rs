use std::cmp::Ordering;

use crate::pipeline::{Position, Rejection};

pub fn apply(text: &str) -> Result<String, Rejection> {
    let java_whitespace = |c: char| {
        matches!(
            c,
            '\t' | '\n'
                | '\u{b}'
                | '\u{c}'
                | '\r'
                | '\u{1c}'..='\u{1f}'
                | ' '
                | '\u{1680}'
                | '\u{2000}'..='\u{2006}'
                | '\u{2008}'..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{205f}'
                | '\u{3000}'
        )
    };
    let mut lines: Vec<&str> = Vec::new();
    let mut rest = text;
    while rest.chars().any(|c| !java_whitespace(c)) {
        let end = rest
            .char_indices()
            .find(|&(_, c)| matches!(c, '\n' | '\u{2028}' | '\u{2029}' | '\u{85}'))
            .map(|(i, c)| (i, i + c.len_utf8()));
        match end {
            Some((line_end, next)) => {
                lines.push(&rest[..line_end]);
                rest = &rest[next..];
            }
            None => {
                lines.push(rest);
                rest = "";
            }
        }
    }

    let mut imports: Vec<&str> = Vec::new();
    let mut first = 0;
    let mut last = 0;
    let mut in_comment = false;
    for (index, line) in lines.iter().enumerate() {
        in_comment |= line.contains("/*");
        if in_comment && line.contains("*/") {
            in_comment = false;
            if !line.contains("/*") {
                continue;
            }
        }
        if let Some(rest) = line.strip_prefix("import ") {
            let dot = line.find('.').map(|i| line[..i].encode_utf16().count());
            match dot {
                Some(position) if position > 7 => {}
                _ => continue,
            }
            if first == 0 {
                first = index + 1;
            }
            last = index + 1;
            let name = rest.find(';').map_or(rest, |end| &rest[..end]);
            if !in_comment && !imports.contains(&name) {
                imports.push(name);
            }
        }
        let scope = line.find('{').map(|brace| line[..brace].contains("//"));
        if let (false, Some(false)) = (in_comment, scope) {
            break;
        }
    }

    let compare = |a: &&str, b: &&str| {
        let (wild_a, wild_b) = (a.find('*'), b.find('*'));
        let utf16 = a.encode_utf16().cmp(b.encode_utf16());
        match (wild_a, wild_b) {
            (Some(_), Some(_)) | (None, None) => utf16,
            (Some(prefix), None) | (None, Some(prefix)) => {
                let same_prefix = a.get(..prefix).is_some_and(|p| b.get(..prefix) == Some(p));
                match (same_prefix, wild_a) {
                    (false, _) => utf16,
                    (true, Some(_)) => Ordering::Less,
                    (true, None) => Ordering::Greater,
                }
            }
        }
    };
    let (mut statics, mut others): (Vec<&str>, Vec<&str>) =
        imports.iter().partition(|name| name.starts_with("static "));
    statics.sort_by(compare);
    others.sort_by(compare);
    let groups: Vec<&Vec<&str>> = [&statics, &others]
        .into_iter()
        .filter(|g| !g.is_empty())
        .collect();
    let sorted: Vec<String> = groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|name| format!("import {name};\n"))
                .collect::<String>()
        })
        .collect();
    let block = sorted.join("\n");
    let original: String = imports
        .iter()
        .map(|name| format!("import {name};\n"))
        .collect();
    if groups.len() == 1 && block == original {
        return Ok(text.to_owned());
    }
    if text.is_empty() {
        return Ok(String::new());
    }

    let mut out = String::new();
    for (index, line) in lines.iter().enumerate() {
        let number = index + 1;
        match (number >= first && number <= last, number == first) {
            (true, true) => out.push_str(&block),
            (true, false) => {}
            (false, _) => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    if !text.ends_with('\n') {
        out.pop().ok_or_else(|| Rejection {
            position: Position::Offset(0),
            message: "import order failed".to_owned(),
        })?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_apply_with_import_blocks_then_returns_them_sorted_as_spotless_does() {
        let cases = [
            (
                r#"import b.B;
class A {}
import a.A;
"#,
                Some(
                    r#"import b.B;
class A {}
import a.A;
"#,
                ),
            ),
            (
                r#"import b.B;
import a.A;
@Deprecated
class A {}
"#,
                Some(
                    r#"import a.A;
import b.B;
@Deprecated
class A {}
"#,
                ),
            ),
            (
                r#"import b.B;
/* import c.C; */
import a.A;
class A {}
"#,
                Some(
                    r#"import a.A;
import b.B;
class A {}
"#,
                ),
            ),
            (
                r#"class A {}
import b.B;
import a.A;
"#,
                Some(
                    r#"class A {}
import b.B;
import a.A;
"#,
                ),
            ),
            (
                r#"import b.a;
import B.a;
import a.B;
import a.b;
import a_b.C;
import a1.C;
class A {}
"#,
                Some(
                    r#"import B.a;
import a.B;
import a.b;
import a1.C;
import a_b.C;
import b.a;
class A {}
"#,
                ),
            ),
            (
                r#"import b.B;
import a.A;
/** doc */
class A {}
"#,
                Some(
                    r#"import a.A;
import b.B;
/** doc */
class A {}
"#,
                ),
            ),
            (
                r#"import b.B; // why
import a.A;
class A {}
"#,
                Some(
                    r#"import a.A;
import b.B;
class A {}
"#,
                ),
            ),
            (
                r#"import module java.base;
import a.A;
class A {}
"#,
                Some(
                    r#"import a.A;
import module java.base;
class A {}
"#,
                ),
            ),
            (
                r#"import b.B;
/*
 import c.C;
*/
import a.A;
class A {}
"#,
                Some(
                    r#"import a.A;
import b.B;
class A {}
"#,
                ),
            ),
            (
                r#"import java.util.Map;
import java.util.Map.Entry;
import java.util.MapX;
import java.util.Mapa;
class A {}
"#,
                Some(
                    r#"import java.util.Map;
import java.util.Map.Entry;
import java.util.MapX;
import java.util.Mapa;
class A {}
"#,
                ),
            ),
            (
                r#"package p;
import b.B;
import a.A;
class A {}
"#,
                Some(
                    r#"package p;
import a.A;
import b.B;
class A {}
"#,
                ),
            ),
            (
                r#"import static b.B.c;
import static a.A.d;

class A {}
"#,
                Some(
                    r#"import static a.A.d;
import static b.B.c;

class A {}
"#,
                ),
            ),
            (
                r#"import b.B;
import a.A;



class A {}
"#,
                Some(
                    r#"import a.A;
import b.B;



class A {}
"#,
                ),
            ),
            (
                r#"import java.util.List; import java.io.File;
class A {}
"#,
                Some(
                    r#"import java.util.List; import java.io.File;
class A {}
"#,
                ),
            ),
            (
                r#"import  java.util.List ;
import java.io.File;
class A {}
"#,
                Some(
                    r#"import  java.util.List ;
import java.io.File;
class A {}
"#,
                ),
            ),
            (
                r#"import static a.B.*;
import static a.B.c;
import static a.A.z;
class A {}
"#,
                Some(
                    r#"import static a.A.z;
import static a.B.*;
import static a.B.c;
class A {}
"#,
                ),
            ),
            (
                r#"	import b.B;
import a.A;
class A {}
"#,
                Some(
                    r#"	import b.B;
import a.A;
class A {}
"#,
                ),
            ),
            (
                r#"import java.io.File;
import java.util.List;

class A {}
"#,
                Some(
                    r#"import java.io.File;
import java.util.List;

class A {}
"#,
                ),
            ),
            (
                r#"import java.util.List;


import java.io.File;

import a.B;
class A {}
"#,
                Some(
                    r#"import a.B;
import java.io.File;
import java.util.List;
class A {}
"#,
                ),
            ),
            (
                r#"import java.util.List;
// keep
import java.io.File;

class A {}
"#,
                Some(
                    r#"import java.io.File;
import java.util.List;

class A {}
"#,
                ),
            ),
            (
                r#"import java.util.List;
import java.util.List;
import java.io.File;

class A {}
"#,
                Some(
                    r#"import java.io.File;
import java.util.List;

class A {}
"#,
                ),
            ),
            (
                r#"import java.util.Map.Entry;
import java.util.Map;
import java.util.AbstractMap.SimpleEntry;

class A {}
"#,
                Some(
                    r#"import java.util.AbstractMap.SimpleEntry;
import java.util.Map;
import java.util.Map.Entry;

class A {}
"#,
                ),
            ),
            (
                r#"package a;

class A {}
"#,
                Some(
                    r#"package a;

class A {}
"#,
                ),
            ),
            (
                r#"import z.Z;
import a.A;
class C {}
"#,
                Some(
                    r#"import a.A;
import z.Z;
class C {}
"#,
                ),
            ),
            (
                r#"import static org.junit.Assert.assertTrue;
import java.util.List;
import static java.lang.Math.max;
import com.google.common.collect.ImmutableList;

class A {}
"#,
                Some(
                    r#"import static java.lang.Math.max;
import static org.junit.Assert.assertTrue;

import com.google.common.collect.ImmutableList;
import java.util.List;

class A {}
"#,
                ),
            ),
            (
                r#"package a;

import java.util.Map;
import java.util.List;
import java.io.File;

class A {}
"#,
                Some(
                    r#"package a;

import java.io.File;
import java.util.List;
import java.util.Map;

class A {}
"#,
                ),
            ),
            (
                r#"import java.util.*;
import java.io.File;
import java.util.List;
import javax.swing.*;

class A {}
"#,
                Some(
                    r#"import java.io.File;
import java.util.*;
import java.util.List;
import javax.swing.*;

class A {}
"#,
                ),
            ),
            (r#"  "#, None),
            (r#""#, Some(r#""#)),
            (
                "import b.B;\u{2028}import a.A;\u{2028}class A {}\n",
                Some(
                    r#"import a.A;
import b.B;
class A {}
"#,
                ),
            ),
        ];
        for (input, expected) in cases {
            let output = apply(input).ok();

            assert_eq!(output.as_deref(), expected, "{input:?}");
        }
    }
}
