mod cli;
#[expect(
    dead_code,
    reason = "statement headers, hugged lambdas and text blocks reach the visitor in tasks 120 to 150"
)]
mod gjf;
mod lexer;
mod parser;
mod profile;
mod steps;
mod syntax;

use cli::{Outcome, Response};
use std::io::{Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut input = Vec::new();
    let outcome = match std::io::stdin().read_to_end(&mut input) {
        Ok(_) => std::thread::Builder::new()
            .stack_size(1 << 30)
            .spawn(move || cli::run(&args, &input))
            .map_err(|e| e.to_string())
            .and_then(|worker| worker.join().map_err(|_| "panicked".to_owned()))
            .unwrap_or_else(|e| Outcome::Internal(format!("jeer: internal error: {e}\n"))),
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
