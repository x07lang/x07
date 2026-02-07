use anyhow::Result;
use clap::Args;

#[derive(Debug, Args)]
pub struct GuideArgs {}

pub fn cmd_guide(
    machine: &crate::reporting::MachineArgs,
    _args: GuideArgs,
) -> Result<std::process::ExitCode> {
    let contents = x07c::compile::guide_md();
    if let Some(path) = machine.out.as_deref() {
        crate::reporting::write_bytes(path, contents.as_bytes())?;
        return Ok(std::process::ExitCode::SUCCESS);
    }
    print!("{contents}");
    Ok(std::process::ExitCode::SUCCESS)
}
