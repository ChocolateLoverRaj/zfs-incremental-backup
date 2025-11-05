use std::{io, process::ExitStatus};

#[allow(unused)]
#[derive(Debug)]
pub enum CommandError {
    Io(io::Error),
    ExitStatus(ExitStatus),
}
