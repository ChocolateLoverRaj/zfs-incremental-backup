use std::{io, process::ExitStatus};

#[derive(Debug)]
pub enum CommandError {
    Io(io::Error),
    ExitStatus(ExitStatus),
}
