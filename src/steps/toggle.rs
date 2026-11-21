pub struct FenceRemoved;

const OFF: &str = "spotless:off";
const ON: &str = "spotless:on";

fn fences(text: &str) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    let mut search = 0;
    while let Some(off) = text[search..].find(OFF) {
        let inside = search + off + OFF.len();
        match text[inside..].find(ON) {
            Some(on) => {
                regions.push((inside, inside + on));
                search = inside + on + ON.len();
            }
            None => break,
        }
    }
    regions
}

pub fn preserving<F>(inner: F) -> impl Fn(&str) -> Result<String, FenceRemoved>
where
    F: Fn(&str) -> String,
{
    move |text| {
        let preserved: Vec<&str> = fences(text)
            .iter()
            .map(|&(start, end)| &text[start..end])
            .collect();
        let formatted = inner(text);
        let regions = fences(&formatted);
        match (preserved.is_empty(), regions.len() == preserved.len()) {
            (true, _) => Ok(formatted),
            (false, false) => Err(FenceRemoved),
            (false, true) => {
                let mut out = String::with_capacity(formatted.len());
                let mut position = 0;
                for (&(start, end), original) in regions.iter().zip(preserved) {
                    out.push_str(&formatted[position..start]);
                    out.push_str(original);
                    position = end;
                }
                out.push_str(&formatted[position..]);
                Ok(out)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_preserving_with_a_step_that_removes_a_fence_then_returns_fence_removed() {
        let remove_fences = |text: &str| text.replace("spotless:on", "");
        let input = "a\n// spotless:off\nb  \n// spotless:on\n";

        let output = preserving(remove_fences)(input).ok();

        assert_eq!(output, None);
    }

    #[test]
    fn when_preserving_trimming_with_fences_then_returns_fenced_text_untouched() {
        let cases = [
            (
                r#"class A {  
// spotless:offspotless:on
}
"#,
                Some(
                    r#"class A {
// spotless:offspotless:on
}
"#,
                ),
            ),
            (
                r#"class A { int a;   /* spotless:off */ int b;   /* spotless:on */ int c;   
}
"#,
                Some(
                    r#"class A { int a;   /* spotless:off */ int b;   /* spotless:on */ int c;
}
"#,
                ),
            ),
            (
                r#"class A {  
// spotless:off
// spotless:off
int a;  
// spotless:on
int b;  
}
"#,
                Some(
                    r#"class A {
// spotless:off
// spotless:off
int a;  
// spotless:on
int b;
}
"#,
                ),
            ),
            (
                r#"class A {  
// spotless:off
int a;  
// spotless:on  
// spotless:on
int b;  
}
"#,
                Some(
                    r#"class A {
// spotless:off
int a;  
// spotless:on
// spotless:on
int b;
}
"#,
                ),
            ),
            (
                r#"class A {
  /* spotless:off */
  int a;  
  /* spotless:on */
  int b;  
}
"#,
                Some(
                    r#"class A {
  /* spotless:off */
  int a;  
  /* spotless:on */
  int b;
}
"#,
                ),
            ),
            (
                r#"class A {
  // @formatter:off
  int x;  
  // @formatter:on
}
"#,
                Some(
                    r#"class A {
  // @formatter:off
  int x;
  // @formatter:on
}
"#,
                ),
            ),
            (
                r#"class A {
  String s = "spotless:off";  
  int x;  
}
"#,
                Some(
                    r#"class A {
  String s = "spotless:off";
  int x;
}
"#,
                ),
            ),
            (
                r#"class A {   
  int x;  
}
"#,
                Some(
                    r#"class A {
  int x;
}
"#,
                ),
            ),
            (
                r#"class A {
  // spotless:off
  int x = 1;   
  // spotless:on
  int y = 2;   
}
"#,
                Some(
                    r#"class A {
  // spotless:off
  int x = 1;   
  // spotless:on
  int y = 2;
}
"#,
                ),
            ),
            (
                r#"class A {  
  // spotless:off
  int x;   
}
"#,
                Some(
                    r#"class A {
  // spotless:off
  int x;
}
"#,
                ),
            ),
            (
                r#"class A {  
  // spotless:on
  int x;  
}
"#,
                Some(
                    r#"class A {
  // spotless:on
  int x;
}
"#,
                ),
            ),
            (
                r#"class A {
  // spotless:off   
  int x;  
  // spotless:on   
}
"#,
                Some(
                    r#"class A {
  // spotless:off   
  int x;  
  // spotless:on
}
"#,
                ),
            ),
            (
                r#"class A {
  // spotless:off
  int a;  
  // spotless:on
  int b;  
  // spotless:off
  int c;  
  // spotless:on
}
"#,
                Some(
                    r#"class A {
  // spotless:off
  int a;  
  // spotless:on
  int b;
  // spotless:off
  int c;  
  // spotless:on
}
"#,
                ),
            ),
        ];
        for (input, expected) in cases {
            let output = preserving(crate::steps::trim_trailing_whitespace::apply)(input).ok();

            assert_eq!(output.as_deref(), expected, "{input:?}");
        }
    }
}
