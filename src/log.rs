use std::os::raw::c_char;

use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};
use obs_sys_rs::{_bindgen_ty_1, LOG_DEBUG, LOG_ERROR, LOG_INFO, LOG_WARNING, blog};

/// A bridge from the [`log`] crate to OBS's logging subsystem.
///
/// OBS exposes four log levels (`error`, `warning`, `info`, `debug`), and
/// the `debug` level is only emitted in debug builds of OBS. To make
/// lower-level Rust logs visible in production OBS builds, configure the
/// logger with [`Logger::with_promote_debug`] to forward `debug` and
/// `trace` records as `info`.
///
/// Plugins are free to use any [`log::Log`] implementation, but routing
/// through OBS has the advantage that records are captured in the OBS
/// log file in addition to the console.
///
/// # Examples
///
/// Install a logger with default settings:
///
/// ```compile_fail
/// let _ = Logger::new().init();
/// ```
pub struct Logger {
    max_level: LevelFilter,
    promote_debug: bool,
}

impl Default for Logger {
    fn default() -> Self {
        Self {
            max_level: LevelFilter::Trace,
            promote_debug: false,
        }
    }
}

impl Logger {
    /// Creates a new logger with [`LevelFilter::Trace`] as the maximum
    /// level and debug-log promotion disabled.
    #[must_use = "You must call init() to begin logging"]
    pub fn new() -> Self {
        Self::default()
    }

    /// Installs this logger as the global [`log`] sink.
    ///
    /// Returns [`SetLoggerError`] if another logger has already been
    /// installed.
    pub fn init(self) -> Result<(), SetLoggerError> {
        log::set_max_level(self.max_level);
        log::set_boxed_logger(Box::new(self))?;
        Ok(())
    }

    /// Configures whether [`Level::Debug`] and [`Level::Trace`] records
    /// are forwarded to OBS's `info` channel.
    ///
    /// Useful when targeting release builds of OBS, which suppress the
    /// `debug` channel.
    #[must_use = "You must call init() to begin logging"]
    pub fn with_promote_debug(mut self, promote_debug: bool) -> Self {
        self.promote_debug = promote_debug;
        self
    }

    /// Sets the maximum log level the logger will forward.
    #[must_use = "You must call init() to begin logging"]
    pub fn with_max_level(mut self, max_level: LevelFilter) -> Self {
        self.max_level = max_level;
        self
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.max_level >= metadata.level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = record.level();
        let native_level = to_native_level(level, self.promote_debug);
        let target = if !record.target().is_empty() {
            record.target()
        } else {
            record.module_path().unwrap_or_default()
        };

        let line = if self.promote_debug && level <= Level::Debug {
            format!("({}) [{}] {}\0", level, target, record.args())
        } else {
            format!("[{}] {}\0", target, record.args())
        };

        unsafe {
            blog(
                native_level as i32,
                c"%s".as_ptr(),
                line.as_ptr() as *const c_char,
            );
        }
    }

    fn flush(&self) {
        // No need to flush
    }
}

fn to_native_level(level: Level, promote_debug: bool) -> _bindgen_ty_1 {
    match level {
        Level::Error => LOG_ERROR,
        Level::Warn => LOG_WARNING,
        Level::Info => LOG_INFO,
        _ => {
            if promote_debug {
                // Debug logs are only enabled in debug builds of OBS, make them accessible as
                // info if needed
                LOG_INFO
            } else {
                LOG_DEBUG
            }
        }
    }
}
