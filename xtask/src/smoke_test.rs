use std::fs;
use std::io::{self, Read};
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
        (Ok(passed), Ok(())) => {
            println!("Smoke test passed: {program}, {} bytes", passed.bytes);
            // A passing app may still have reported something, such as a folder it could not delete.
            if !passed.stderr.trim().is_empty() {
                eprintln!("warning: {program} wrote to stderr:\n{}", passed.stderr);
            }
            Ok(())
        }
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(close_error)) => {
            eprintln!("error: {close_error:#}");
            Err(error)
        }
    }
}

/// What a passing run saved, and what the program wrote to stderr meanwhile.
struct PassedRun {
    bytes: usize,
    stderr: String,
}

fn smoke_test_in(folder: &Path, program: &str, arguments: &[&str]) -> Result<PassedRun> {
    let input_path = folder.join("smoke-input.asset");
    let output_path = folder.join("smoke-output.asset");
    let input = smoke_input()?;
    fs::write(&input_path, &input)
        .with_context(|| format!("could not write {}", input_path.display()))?;

    let mut command = Command::new(program);
    command
        .args(arguments)
        .arg(SMOKE_TEST_FLAG)
        .arg(&input_path)
        .arg(&output_path)
        .stdin(Stdio::null())
        .stderr(Stdio::piped());
    start_in_own_process_group(&mut command);
    let mut child = command
        .spawn()
        .with_context(|| format!("could not start {program}"))?;
    let stderr = read_stderr(&mut child);
    let exit = wait_for_exit(&mut child, program);
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

    let status = exit.map_err(with_stderr)?;
    if !status.success() {
        return Err(with_stderr(format!("{program} exited with {status}")));
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
    Ok(PassedRun {
        bytes: output.len(),
        stderr,
    })
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

/// The child's exit status, or why there is none. A child that runs longer than `XTASK_SMOKE_TIMEOUT`, or that
/// can no longer be waited for, is stopped; on Unix together with every process in its group
/// (`start_in_own_process_group`).
fn wait_for_exit(child: &mut Child, program: &str) -> Result<ExitStatus, String> {
    let started = Instant::now();
    loop {
        let problem = match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() > XTASK_SMOKE_TIMEOUT => format!(
                "{program} did not finish within {} s",
                XTASK_SMOKE_TIMEOUT.as_secs()
            ),
            Ok(None) => {
                thread::sleep(EXIT_POLL_INTERVAL);
                continue;
            }
            Err(error) => format!("could not wait for {program}: {error}"),
        };
        return Err(match stop(child) {
            Ok(()) => format!("{problem}, so it was stopped"),
            Err(error) => format!("{problem}, and stopping it failed: {error}"),
        });
    }
}

/// On Unix the program leads a process group of its own, so that stopping it reaches the processes it started
/// too: an AppImage's runtime runs the app as its child, and killing only the runtime would leave the app
/// running.
#[cfg(unix)]
fn start_in_own_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

/// Windows runs the packaged exe directly, with no launcher in between.
#[cfg(not(unix))]
fn start_in_own_process_group(_command: &mut Command) {}

/// Kills the child's process group, whose id is the child's own (`start_in_own_process_group`), then reaps the
/// child.
#[cfg(unix)]
fn stop(child: &mut Child) -> io::Result<()> {
    use rustix::process::{Pid, Signal, kill_process_group};
    kill_process_group(Pid::from_child(child), Signal::KILL)?;
    child.wait()?;
    Ok(())
}

#[cfg(not(unix))]
fn stop(child: &mut Child) -> io::Result<()> {
    child.kill()?;
    child.wait()?;
    Ok(())
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

    /// A shell that starts a background `sleep` stands in for an AppImage runtime and its app: once both are
    /// stopped, nothing holds the stdout pipe they share any more.
    #[cfg(unix)]
    #[test]
    fn stopping_ends_the_processes_the_program_started() {
        use std::io::{BufRead, BufReader};

        let mut command = Command::new("sh");
        command
            .args(["-c", "sleep 600 & echo started; wait"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped());
        start_in_own_process_group(&mut command);
        let mut child = command.spawn().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut first_line = String::new();
        stdout.read_line(&mut first_line).unwrap();
        assert_eq!(first_line.trim_end(), "started");

        stop(&mut child).unwrap();

        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut rest = Vec::new();
            sender
                .send(stdout.read_to_end(&mut rest).map(|_| rest))
                .expect("the test waits for stdout to close");
        });
        let rest = receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the background sleep was stopped, so stdout closes");
        assert_eq!(rest.unwrap(), b"");
    }
}
