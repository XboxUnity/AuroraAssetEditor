use super::error_log::ErrorLog;
use super::log_write_failure::report_log_write_failure;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};

pub const SUPPAFTP_TARGET: &str = "suppaftp";

/// suppaftp logs every control-channel reply (`CC IN`, which carries Aurora's password hashes), the PASS command
/// (`CC OUT`) and the user name below Warn, so nothing under this level may reach `error.log`.
/// The cap holds even if the bridge's own level is raised later.
pub const SUPPAFTP_MAX_LEVEL: LevelFilter = LevelFilter::Warn;

const BRIDGE_MAX_LEVEL: LevelFilter = LevelFilter::Warn;

/// Sends `log` records up to its level (Warn) to the error log.
pub struct LogBridge {
    log: ErrorLog,
    max_level: LevelFilter,
}

impl LogBridge {
    pub fn new(log: ErrorLog) -> LogBridge {
        LogBridge::with_max_level(log, BRIDGE_MAX_LEVEL)
    }

    pub fn with_max_level(log: ErrorLog, max_level: LevelFilter) -> LogBridge {
        LogBridge { log, max_level }
    }

    pub fn install(log: ErrorLog) -> Result<(), SetLoggerError> {
        log::set_boxed_logger(Box::new(LogBridge::new(log)))?;
        log::set_max_level(BRIDGE_MAX_LEVEL);
        Ok(())
    }

    fn level_cap_for(&self, target: &str) -> LevelFilter {
        let from_suppaftp = target == SUPPAFTP_TARGET
            || target
                .strip_prefix(SUPPAFTP_TARGET)
                .is_some_and(|rest| rest.starts_with("::"));
        if from_suppaftp {
            SUPPAFTP_MAX_LEVEL.min(self.max_level)
        } else {
            self.max_level
        }
    }
}

impl Log for LogBridge {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level_cap_for(metadata.target())
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = record.args().to_string();
        if let Err(write_error) = self.log.record_message(record.target(), &message) {
            report_log_write_failure(&self.log, &write_error);
        }
    }

    fn flush(&self) {}
}
