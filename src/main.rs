mod cli;
mod profile;
mod steps;

use cli::{Outcome, Response};
use std::io::{Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut input = Vec::new();
    let outcome = match std::io::stdin().read_to_end(&mut input) {
        Ok(_) => std::panic::catch_unwind(|| cli::run(&args, &input))
            .unwrap_or_else(|_| Outcome::Internal("jeer: internal error\n".to_owned())),
        Err(e) => Outcome::Internal(format!("jeer: cannot read stdin: {e}\n")),
    };
    let response = Response::from(outcome);
    let written = std::io::stdout()
        .write_all(response.stdout.as_bytes())
        .and_then(|()| std::io::stdout().flush());
    eprint!("{}", response.stderr);
    match written {
        Ok(()) => std::process::exit(response.code),
        Err(e) => {
            eprintln!("jeer: cannot write stdout: {e}");
            std::process::exit(Response::from(Outcome::Internal(String::new())).code);
        }
    }
}
