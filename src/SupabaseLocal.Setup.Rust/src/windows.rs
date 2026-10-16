use anyhow::{Context, Result, ensure};
use std::{path::Path, process::Command};

fn powershell(script: &str, args: &[&std::ffi::OsStr]) -> Result<()> {
    use std::os::windows::process::CommandExt;
    let mut file = tempfile::Builder::new().suffix(".ps1").tempfile()?;
    use std::io::Write;
    file.write_all(script.as_bytes())?;
    // Windows PowerShell cannot read the delete-on-close handle owned by
    // NamedTempFile. Keep the path alive, but close its handle before spawning.
    let path = file.into_temp_path();
    let output = Command::new("powershell.exe")
        .creation_flags(0x08000000)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(path.as_os_str())
        .args(args)
        .output()?;
    ensure!(
        output.status.success(),
        "Windows integration failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn powershell_errors_include_the_underlying_reason() {
        let error = powershell("throw 'integration-test-reason'", &[]).unwrap_err();
        assert!(error.to_string().contains("integration-test-reason"));
    }

    #[test]
    fn registration_creates_correct_shortcuts_and_registry_in_isolated_scope() {
        let temporary = tempfile::tempdir().unwrap();
        // WScript.Shell expands the user's 8.3 TEMP path when saving shortcuts.
        let base = std::fs::canonicalize(temporary.path()).unwrap();
        let base = std::path::PathBuf::from(base.to_string_lossy().trim_start_matches(r"\\?\"));
        let root = base.join("installation with spaces");
        let version = root.join("versions").join("test");
        let programs = base.join("test programs");
        std::fs::create_dir_all(&version).unwrap();
        let executable = version.join("SupabaseLocal.App.exe");
        std::fs::write(&executable, b"test placeholder, never executed").unwrap();
        let key = format!(
            "HKCU:\\Software\\SupabaseLocalRustInstallerTests\\{}",
            temporary.path().file_name().unwrap().to_string_lossy()
        );
        let registration = powershell(
            include_str!("register.ps1"),
            &[
                version.as_os_str(),
                root.as_os_str(),
                programs.as_os_str(),
                std::ffi::OsStr::new(&key),
            ],
        );
        let validation = powershell(
            r#"
param([string]$Programs, [string]$RegistryPath, [string]$Exe, [string]$Root)
$ErrorActionPreference = 'Stop'
try {
    $values = Get-ItemProperty -LiteralPath $RegistryPath
    if ($values.InstallerOwner -ne 'SupabaseLocal.NativeWindows.v1' -or $values.InstallLocation -ne $Root) { throw 'Wrong registration ownership' }
    if ($values.UninstallString -ne ('"' + $Exe + '" --uninstall')) { throw 'Wrong uninstall command' }
    $shell = New-Object -ComObject WScript.Shell
    $app = $shell.CreateShortcut((Join-Path $Programs 'Supabase Local.lnk'))
    $uninstall = $shell.CreateShortcut((Join-Path $Programs 'Uninstall Supabase Local.lnk'))
    if ($app.TargetPath -ne $Exe -or $app.Arguments -ne '' -or $uninstall.TargetPath -ne $Exe -or $uninstall.Arguments -ne '--uninstall') { throw ('Wrong shortcut target or arguments: app=[' + $app.TargetPath + '][' + $app.Arguments + ']; uninstall=[' + $uninstall.TargetPath + '][' + $uninstall.Arguments + ']; expected=[' + $Exe + ']') }
} finally {
    Remove-Item -LiteralPath $RegistryPath -ErrorAction SilentlyContinue
}
"#,
            &[
                programs.as_os_str(),
                std::ffi::OsStr::new(&key),
                executable.as_os_str(),
                root.as_os_str(),
            ],
        );
        registration.unwrap();
        validation.unwrap();
    }
    #[test]
    fn temporary_powershell_script_runs_with_space_containing_arguments() {
        powershell("param([string]$Value)\nif ($Value -ne 'path with spaces') { throw 'Argument was changed' }\nWrite-Output 'done'", &[std::ffi::OsStr::new("path with spaces")]).unwrap();
    }
}

pub fn prerequisites() -> Result<()> {
    powershell(include_str!("prerequisites.ps1"), &[]).context("Checking Microsoft C++ runtime")
}

pub fn register(version: &Path, root: &Path) -> Result<()> {
    powershell(
        include_str!("register.ps1"),
        &[version.as_os_str(), root.as_os_str()],
    )
    .context("Registering Windows shortcuts and Installed apps entry")
}

pub fn launch(version: &Path) -> Result<()> {
    std::process::Command::new(version.join("SupabaseLocal.App.exe"))
        .current_dir(version)
        .spawn()?;
    Ok(())
}
