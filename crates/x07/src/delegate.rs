use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, ExitCode, ExitStatus, Output, Stdio};

use anyhow::{Context, Result};

#[derive(Debug)]
pub enum DelegateOutput {
    Exited(ExitStatus),
    NotFound,
}

#[derive(Debug)]
pub enum DelegateCaptured {
    Output(Output),
    NotFound,
}

pub fn exit_code_from_status(status: &ExitStatus) -> ExitCode {
    let code = status.code().unwrap_or(1);
    if code < 0 {
        return ExitCode::from(1);
    }
    if code > 255 {
        return ExitCode::from(1);
    }
    ExitCode::from(code as u8)
}

pub fn run_inherit(name: &str, args: &[OsString]) -> Result<DelegateOutput> {
    let mut cmd = Command::new(name);
    cmd.args(args);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    match cmd.status() {
        Ok(status) => Ok(DelegateOutput::Exited(status)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(DelegateOutput::NotFound),
        Err(err) => Err(err).with_context(|| format!("spawn {name}")),
    }
}

pub fn run_capture(name: &str, args: &[OsString], cwd: Option<&Path>) -> Result<DelegateCaptured> {
    let mut cmd = Command::new(name);
    cmd.args(args);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }

    match cmd.output() {
        Ok(out) => Ok(DelegateCaptured::Output(out)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(DelegateCaptured::NotFound),
        Err(err) => Err(err).with_context(|| format!("run {name}")),
    }
}
