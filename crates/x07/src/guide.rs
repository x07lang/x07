use anyhow::Result;
use clap::Args;

#[derive(Debug, Args)]
pub struct GuideArgs {}

pub fn cmd_guide(_args: GuideArgs) -> Result<std::process::ExitCode> {
    print!("{}", x07c::compile::guide_md());
    Ok(std::process::ExitCode::SUCCESS)
}
