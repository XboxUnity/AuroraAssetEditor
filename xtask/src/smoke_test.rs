use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use aurora_asset::{AssetFile, AssetSlot, Compression};

use crate::notices::repository_root;

pub const USAGE_LINE: &str = "cargo xtask smoke-test <program> [<arguments>...]";

// Mirrors the editor's `SMOKE_TEST_FLAG`; xtask does not depend on the editor.
const SMOKE_TEST_FLAG: &str = "--smoke-test";
/// Longer than the app's own 60 s limit, so the app reports its own timeout first.
const XTASK_SMOKE_TIMEOUT: Duration = Duration::from_secs(90);
const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(100);
/// How long stderr may stay open after the program ended, held by a process it started.
const STDERR_WAIT: Duration = Duration::from_secs(5);
const COVER_SOURCE: &str = "fixtures/codec/inputs/ph_cover.png";

/// Runs `<program> <arguments...> --smoke-test <in> <out>`, where the arguments are a launcher's (such as an
/// AppImage's), and requires the app to save the cover asset it loaded byte for byte.
pub fn run(command: &[&str]) -> Result<()> {
    let Some((program, arguments)) = command.split_first() else {
        bail!("usage: {USAGE_LINE}");
    };
    let folder = tempfile::tempdir().context("could not create a temporary folder")?;
    let result = smoke_test_in(folder.path(), program, arguments);
    let closed = folder
        .close()
        .context("could not delete the smoke test's temporary folder");
    match (result, closed) {
        (Ok(bytes), Ok(())) => {
            println!("Smoke test passed: {program}, {bytes} bytes");
            Ok(())
        }
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(close_error)) => {
            eprintln!("error: {close_error:#}");
            Err(error)
        }
    }
}

/// Runs the smoke test with its files in `folder`; the number of bytes saved when it passes.
fn smoke_test_in(folder: &Path, program: &str, arguments: &[&str]) -> Result<usize> {
    let input_path = folder.join("smoke-input.asset");
    let output_path = folder.join("smoke-output.asset");
    let input = smoke_input()?;
    fs::write(&input_path, &input)
        .with_context(|| format!("could not write {}", input_path.display()))?;

    let mut child = Command::new(program)
        .args(arguments)
        .arg(SMOKE_TEST_FLAG)
        .arg(&input_path)
        .arg(&output_path)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("could not start {program}"))?;
    let stderr = read_stderr(&mut child);
    let exit = wait_for_exit(&mut child, program)?;
    let stderr = match stderr.recv_timeout(STDERR_WAIT) {
        Ok(text) => text,
        Err(RecvTimeoutError::Timeout) => format!(
            "(stderr was still open {} s after the program ended)",
            STDERR_WAIT.as_secs()
        ),
        Err(RecvTimeoutError::Disconnected) => "(the stderr reader stopped)".to_owned(),
    };
    let with_stderr =
        |message: String| anyhow::anyhow!("{message}\nstderr of {program}:\n{stderr}");

    match exit {
        Exit::TimedOut => {
            return Err(with_stderr(format!(
                "{program} did not finish within {} s and was stopped",
                XTASK_SMOKE_TIMEOUT.as_secs()
            )));
        }
        Exit::Finished(status) if !status.success() => {
            return Err(with_stderr(format!("{program} exited with {status}")));
        }
        Exit::Finished(_) => {}
    }
    let output = fs::read(&output_path).map_err(|error| {
        with_stderr(format!(
            "{program} exited without an error, but its output {} could not be read: {error}",
            output_path.display()
        ))
    })?;
    if output != input {
        return Err(with_stderr(format!(
            "{program} saved {} bytes that differ from the {} bytes it loaded",
            output.len(),
            input.len()
        )));
    }
    Ok(output.len())
}

/// A cover asset made the way the app makes one: the placeholder cover, DXT-compressed.
fn smoke_input() -> Result<Vec<u8>> {
    let path = repository_root().join(COVER_SOURCE);
    let bytes = fs::read(&path).with_context(|| format!("could not read {}", path.display()))?;
    let image = aurora_image::decode_image(&bytes, Some("ph_cover.png"))
        .with_context(|| format!("could not decode {}", path.display()))?;
    let texture = aurora_asset::encode(&aurora_image::encoder_pixels(&image), Compression::Dxt)
        .context("could not encode the cover")?;
    let mut file = AssetFile::new();
    file.set(AssetSlot::Cover, Some(texture));
    Ok(file.to_bytes())
}

/// Reads the child's stderr on its own thread, so a full pipe never stops the child.
fn read_stderr(child: &mut Child) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    let pipe = child.stderr.take();
    thread::spawn(move || {
        let text = match pipe {
            Some(mut pipe) => {
                let mut bytes = Vec::new();
                match pipe.read_to_end(&mut bytes) {
                    Ok(_) => String::from_utf8_lossy(&bytes).into_owned(),
                    Err(error) => format!("(could not read stderr: {error})"),
                }
            }
            None => "(stderr was not captured)".to_owned(),
        };
        // The receiver is gone only when the run already ended without waiting for stderr.
        let _unwanted = sender.send(text);
    });
    receiver
}

enum Exit {
    Finished(ExitStatus),
    TimedOut,
}

/// Stops the child through its own handle when it runs longer than `XTASK_SMOKE_TIMEOUT`.
fn wait_for_exit(child: &mut Child, program: &str) -> Result<Exit> {
    let started = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .with_context(|| format!("could not wait for {program}"))?
        {
            return Ok(Exit::Finished(status));
        }
        if started.elapsed() > XTASK_SMOKE_TIMEOUT {
            child
                .kill()
                .with_context(|| format!("could not stop {program}"))?;
            child
                .wait()
                .with_context(|| format!("could not wait for {program} after stopping it"))?;
            return Ok(Exit::TimedOut);
        }
        thread::sleep(EXIT_POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use aurora_asset::{AssetFile, AssetKind};

    use super::*;

    #[test]
    fn smoke_input_is_a_cover_asset() -> Result<()> {
        let file = AssetFile::parse(&smoke_input()?)?;
        assert_eq!(file.primary_kind(), Some(AssetKind::Cover));
        Ok(())
    }
}
