//! Runs `tools/gdiplus/gdiplus-reference.ps1`, which writes GDI+ reference pixels.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::golden::Paths;

const WINDOWS_ROOT_VARIABLE: &str = "SystemRoot";
const POWERSHELL_PATH: [&str; 3] = ["WindowsPowerShell", "v1.0", "powershell.exe"];
const REFERENCE_SCRIPT: [&str; 3] = ["tools", "gdiplus", "gdiplus-reference.ps1"];

/// Which Windows PowerShell 5.1 runs the tool. The old app ran as a 32-bit process.
#[derive(Clone, Copy)]
pub enum PowerShellHost {
    Bit32,
    Bit64,
}

impl PowerShellHost {
    fn system_directory(self) -> &'static str {
        match self {
            PowerShellHost::Bit32 => "SysWOW64",
            PowerShellHost::Bit64 => "System32",
        }
    }

    fn executable(self) -> Result<PathBuf> {
        let windows = std::env::var_os(WINDOWS_ROOT_VARIABLE)
            .with_context(|| format!("{WINDOWS_ROOT_VARIABLE} is not set"))?;
        Ok(POWERSHELL_PATH.iter().fold(
            PathBuf::from(windows).join(self.system_directory()),
            |path, part| path.join(part),
        ))
    }
}

/// Writes `<stem>.<width>x<height>.argb` into `output_dir` for every file in `input_dir`, at the
/// app's five target sizes unless `targets` lists others (`"256x256"`).
pub fn run_reference_tool(
    paths: &Paths,
    host: PowerShellHost,
    input_dir: &Path,
    output_dir: &Path,
    targets: Option<&str>,
) -> Result<()> {
    let script = REFERENCE_SCRIPT
        .iter()
        .fold(paths.root().to_path_buf(), |path, part| path.join(part));
    let mut command = Command::new(host.executable()?);
    command
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .arg("-InputDir")
        .arg(input_dir)
        .arg("-OutputDir")
        .arg(output_dir);
    if let Some(targets) = targets {
        command.arg("-Targets").arg(targets);
    }
    let output = command
        .output()
        .context("cannot start Windows PowerShell")?;
    if !output.status.success() {
        bail!(
            "the GDI+ reference tool failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}
