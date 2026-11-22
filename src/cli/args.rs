use crate::pipeline::Profile;

pub const USAGE: &str = "usage: jeer --stdin --profile <profile>";

pub enum Invocation {
    FormatStdin(Profile),
}

pub fn parse(args: &[String]) -> Result<Invocation, String> {
    let mut stdin = false;
    let mut profile = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--stdin" => stdin = true,
            "--profile" => {
                let name = rest.next().ok_or("--profile needs a value")?;
                profile = Some(name.parse::<Profile>()?);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    match (stdin, profile) {
        (true, Some(profile)) => Ok(Invocation::FormatStdin(profile)),
        (false, _) => Err("--stdin is required".to_owned()),
        (true, None) => Err("--profile is required".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_parse_with_stdin_and_a_profile_in_any_order_then_returns_format_stdin() {
        let orders: [&[&str]; 2] = [
            &["--stdin", "--profile", "whitespace"],
            &["--profile", "whitespace", "--stdin"],
        ];
        for order in orders {
            let args: Vec<String> = order.iter().map(|a| (*a).to_owned()).collect();

            let invocation = parse(&args);

            assert!(
                matches!(invocation, Ok(Invocation::FormatStdin(Profile::Whitespace))),
                "{order:?}"
            );
        }
    }

    #[test]
    fn when_parse_with_invalid_arguments_then_returns_the_error() {
        let cases: [(&[&str], &str); 5] = [
            (&[], "--stdin is required"),
            (&["--stdin"], "--profile is required"),
            (&["--stdin", "--profile"], "--profile needs a value"),
            (&["--stdin", "--profile", "nope"], "unknown profile: nope"),
            (&["--stdin", "--check"], "unknown argument: --check"),
        ];
        for (args, expected) in cases {
            let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();

            let error = parse(&args).err();

            assert_eq!(error.as_deref(), Some(expected), "{args:?}");
        }
    }
}
