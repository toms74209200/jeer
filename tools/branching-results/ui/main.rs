#![allow(dead_code)]

struct Token;

impl Token {
    fn is_keyword(&self) -> bool {
        true
    }
    fn kind(&self) -> Kind {
        Kind::Keyword
    }
}

enum Kind {
    Keyword,
    Other,
}

fn valid(s: &str) -> bool {
    !s.is_empty()
}

fn find(s: &str) -> Option<usize> {
    s.find('x')
}

fn parse(s: &str) -> Result<u8, ()> {
    s.parse().map_err(|_| ())
}

impl PartialEq for Token {
    fn eq(&self, _: &Token) -> bool {
        true
    }
}

trait Check {
    fn ok(&self) -> bool;
}

impl Check for Token {
    fn ok(&self) -> bool {
        true
    }
}

macro_rules! when_valid {
    ($s:expr) => {
        if valid($s) {}
    };
}

fn flagged(s: &str, t: &Token) {
    if valid(s) {}
    if !valid(s) && t.is_keyword() {}
    while valid(s) {
        break;
    }
    match valid(s) {
        true => {}
        false => {}
    }
    match s {
        "a" if t.is_keyword() => {}
        _ => {}
    }
    let _ = find(s).is_some();
    let _ = parse(s).is_err();
    let _ = find(s) == None;
    let _ = valid(s) != true;
}

fn accepted(s: &str, t: &Token) {
    match t.kind() {
        Kind::Keyword => {}
        Kind::Other => {}
    }
    if let Some(i) = find(s) {
        let _ = i;
    }
    if s.is_empty() {}
    if *t == Token {}
    let ok = valid(s);
    let _ = ok;
    assert!(valid(s));
    let _ = [1, 2].iter().any(|_| valid(s));
    if t.ok() {}
    when_valid!(s);
}

fn main() {
    flagged("x", &Token);
    accepted("x", &Token);
}
