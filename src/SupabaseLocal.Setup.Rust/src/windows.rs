use anyhow::{Result, ensure};
use std::{path::Path, process::Command};

fn powershell(script: &str, args: &[&std::ffi::OsStr]) -> Result<()> {
    use std::os::windows::process::CommandExt;
    let mut file = tempfile::Builder::new().suffix(".ps1").tempfile()?;
    use std::io::Write;
    file.write_all(script.as_bytes())?;
    let status = Command::new("powershell.exe")
        .creation_flags(0x08000000)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(file.path())
        .args(args)
        .status()?;
    ensure!(status.success(), "Windows integration failed ({status})");
    Ok(())
}

pub fn prerequisites() -> Result<()> {
    powershell(include_str!("prerequisites.ps1"), &[])
}

pub fn register(version: &Path, root: &Path) -> Result<()> {
    powershell(
        include_str!("register.ps1"),
        &[version.as_os_str(), root.as_os_str()],
    )
}

pub fn launch(version: &Path) -> Result<()> {
    std::process::Command::new(version.join("SupabaseLocal.App.exe"))
        .current_dir(version)
        .spawn()?;
    Ok(())
}
