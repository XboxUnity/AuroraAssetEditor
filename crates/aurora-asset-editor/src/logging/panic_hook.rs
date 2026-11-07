use super::error_log::ErrorLog;
use super::log_write_failure::report_log_write_failure;
use std::panic::{self, PanicHookInfo};

const PANIC_CONTEXT: &str = "panic";

/// Records every panic in the error log, then runs the hook that was installed before.
pub fn install_panic_hook(log: ErrorLog) {
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if let Err(write_error) = log.record_message(PANIC_CONTEXT, &describe_panic(info)) {
            report_log_write_failure(&log, &write_error);
        }
        previous_hook(info);
    }));
}

fn describe_panic(info: &PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let message = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("panic without a text message");
    match info.location() {
        Some(location) => format!("{message} at {}:{}", location.file(), location.line()),
        None => message.to_string(),
    }
}
