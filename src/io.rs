use std::io::{Read, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Stdin,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Inputs {
    pub stdin: Vec<u8>,
}

pub struct Writes<'a> {
    pub stdout: &'a str,
    pub stderr: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Failure(pub String);

pub fn read(requests: &[Request]) -> Result<Inputs, Failure> {
    let mut stdin = Vec::new();
    let read = match requests.contains(&Request::Stdin) {
        true => std::io::stdin().read_to_end(&mut stdin).map(|_| ()),
        false => Ok(()),
    };
    read.map(|()| Inputs { stdin })
        .map_err(|e| Failure(format!("cannot read stdin: {e}")))
}

pub fn write(writes: &Writes) -> Result<(), Failure> {
    let stdout = std::io::stdout()
        .write_all(writes.stdout.as_bytes())
        .and_then(|()| std::io::stdout().flush())
        .map_err(|e| Failure(format!("cannot write stdout: {e}")));
    eprint!("{}", writes.stderr);
    stdout
}
