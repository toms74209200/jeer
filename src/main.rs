mod cli;
#[expect(dead_code, reason = "text blocks reach the visitor in task 150")]
mod gjf;
mod io;
mod lexer;
mod parser;
mod pipeline;
mod steps;

use cli::{Outcome, Response};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = cli::plan(&args).map_or_else(Outcome::from, |plan| {
        io::read(&plan.requests).map_or_else(
            |io::Failure(e)| Outcome::Internal(format!("jeer: {e}\n")),
            |inputs| {
                std::thread::Builder::new()
                    .stack_size(1 << 30)
                    .spawn(move || cli::run(plan, &inputs))
                    .map_err(|e| e.to_string())
                    .and_then(|worker| worker.join().map_err(|_| "panicked".to_owned()))
                    .unwrap_or_else(|e| Outcome::Internal(format!("jeer: internal error: {e}\n")))
            },
        )
    });
    let response = Response::from(outcome);
    let code = match io::write(&io::Writes {
        stdout: &response.stdout,
        stderr: &response.stderr,
    }) {
        io::Written::Done => response.code,
        io::Written::StdoutFailed | io::Written::StderrFailed => {
            Response::from(Outcome::Internal(String::new())).code
        }
    };
    std::process::exit(code);
}
