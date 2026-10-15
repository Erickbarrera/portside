//! Runs the system tools some steps need (`hdiutil`, `ditto`, `xattr` on macOS).

use std::ffi::OsStr;
use std::io::Write;
use std::process::{Command, Stdio};

use crate::error::{InstallError, Result};

/// Runs `tool` with `args`, optionally feeding `stdin`; fails on a non-zero exit.
pub fn run(tool: &'static str, args: &[&OsStr], stdin: Option<&[u8]>) -> Result<()> {
    let fail = |message: String| InstallError::Tool { tool, message };
    let mut child = Command::new(tool)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| fail(e.to_string()))?;
    if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
        // The tool may exit without reading; that's not an error.
        let _ = pipe.write_all(input);
    }
    let output = child.wait_with_output().map_err(|e| fail(e.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(fail(stderr.trim().chars().take(300).collect()))
    }
}
