use super::error_log::ErrorLog;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};

pub const SUPPAFTP_TARGET: &str = "suppaftp";

/// suppaftp logs every control-channel reply (`CC IN`, which carries Aurora's password hashes), the PASS command
/// (`CC OUT`) and the user name below Warn, so nothing under this level may reach `error.log`.
pub const SUPPAFTP_MAX_LEVEL: LevelFilter = LevelFilter::Warn;

const BRIDGE_MAX_LEVEL: LevelFilter = LevelFilter::Warn;

/// Sends `log` records of level Warn and Error to the error log.
pub struct LogBridge {
    log: ErrorLog,
}

impl LogBridge {
    pub fn new(log: ErrorLog) -> LogBridge {
        LogBridge { log }
    }

    pub fn install(log: ErrorLog) -> Result<(), SetLoggerError> {
        log::set_boxed_logger(Box::new(LogBridge::new(log)))?;
        log::set_max_level(BRIDGE_MAX_LEVEL);
        Ok(())
    }
}

fn level_cap_for(target: &str) -> LevelFilter {
    let from_suppaftp = target == SUPPAFTP_TARGET
        || target
            .strip_prefix(SUPPAFTP_TARGET)
            .is_some_and(|rest| rest.starts_with("::"));
    if from_suppaftp {
        SUPPAFTP_MAX_LEVEL.min(BRIDGE_MAX_LEVEL)
    } else {
        BRIDGE_MAX_LEVEL
    }
}

impl Log for LogBridge {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= level_cap_for(metadata.target())
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = record.args().to_string();
        if let Err(write_error) = self.log.record_message(record.target(), &message) {
            // The error log is the only sink there is, so the failure goes to stderr.
            eprintln!(
                "could not write to {}: {write_error}",
                self.log.path().display()
            );
        }
    }

    fn flush(&self) {}
}
