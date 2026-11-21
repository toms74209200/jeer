mod args;

use args::Invocation;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Unchanged(String),
    Changed(String),
    Usage(String),
    Internal(String),
}

pub struct Response {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

impl From<Outcome> for Response {
    fn from(outcome: Outcome) -> Self {
        let (stdout, stderr, code) = match outcome {
            Outcome::Unchanged(text) => (text, String::new(), 0),
            Outcome::Changed(text) => (text, String::new(), 1),
            Outcome::Usage(message) => (String::new(), message, 4),
            Outcome::Internal(message) => (String::new(), message, 3),
        };
        Response {
            stdout,
            stderr,
            code,
        }
    }
}

pub fn run(args: &[String], input: &[u8]) -> Outcome {
    match args::parse(args) {
        Ok(Invocation::FormatStdin(profile)) => {
            let source = String::from_utf8_lossy(input);
            let formatted = profile.format(&source);
            if formatted.as_bytes() == input {
                Outcome::Unchanged(formatted)
            } else {
                Outcome::Changed(formatted)
            }
        }
        Err(error) => Outcome::Usage(format!("jeer: {error}\n{}\n", args::USAGE)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_from_with_each_outcome_then_returns_its_streams_and_exit_code() {
        let cases = [
            (Outcome::Unchanged("a\n".to_owned()), ("a\n", "", 0)),
            (Outcome::Changed("b\n".to_owned()), ("b\n", "", 1)),
            (Outcome::Usage("usage\n".to_owned()), ("", "usage\n", 4)),
            (Outcome::Internal("broke\n".to_owned()), ("", "broke\n", 3)),
        ];
        for (outcome, (stdout, stderr, code)) in cases {
            let response = Response::from(outcome);

            assert_eq!(
                (
                    response.stdout.as_str(),
                    response.stderr.as_str(),
                    response.code
                ),
                (stdout, stderr, code)
            );
        }
    }

    #[test]
    fn when_run_with_a_profile_then_returns_unchanged_or_changed_with_the_output() {
        let cases = [
            (
                "class A {}\n",
                Outcome::Unchanged("class A {}\n".to_owned()),
            ),
            ("class A {}  ", Outcome::Changed("class A {}\n".to_owned())),
            ("a\r\n", Outcome::Changed("a\n".to_owned())),
            ("a\u{ff}\n", Outcome::Unchanged("a\u{ff}\n".to_owned())),
        ];
        for (input, expected) in cases {
            let args: Vec<String> = ["--stdin", "--profile", "whitespace"]
                .iter()
                .map(|a| (*a).to_owned())
                .collect();

            let outcome = run(&args, input.as_bytes());

            assert_eq!(outcome, expected, "{input:?}");
        }
    }

    #[test]
    fn when_run_with_invalid_utf8_then_returns_it_changed_as_java_decodes_it() {
        let args: Vec<String> = ["--stdin", "--profile", "whitespace"]
            .iter()
            .map(|a| (*a).to_owned())
            .collect();

        let outcome = run(&args, b"a\xff\n");

        assert_eq!(outcome, Outcome::Changed("a\u{fffd}\n".to_owned()));
    }

    #[test]
    fn when_run_with_invalid_arguments_then_returns_usage_with_the_error_and_the_usage() {
        let cases: [(&[&str], &str); 2] = [
            (
                &["--stdin", "--profile", "nope"],
                "jeer: unknown profile: nope\n",
            ),
            (&["--check"], "jeer: unknown argument: --check\n"),
        ];
        for (args, error) in cases {
            let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();

            let outcome = run(&args, b"");

            assert_eq!(
                outcome,
                Outcome::Usage(format!("{error}{}\n", args::USAGE)),
                "{args:?}"
            );
        }
    }
}
