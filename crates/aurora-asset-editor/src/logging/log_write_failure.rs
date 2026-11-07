use super::error_log::ErrorLog;
use std::io::{self, Write};

/// For the log bridge and the panic hook, which cannot return an error: the failure goes to stderr.
pub(super) fn report_log_write_failure(log: &ErrorLog, write_error: &io::Error) {
    let report = writeln!(
        io::stderr(),
        "could not write to {}: {write_error}",
        log.path().display()
    );
    // stderr is the last place to report to; if it fails too, nothing is left to tell.
    drop(report);
}
