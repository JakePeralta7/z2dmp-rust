use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::result::{Result, Error};

/// Supported log-levels.
///
/// Which messages get printed is determined at runtime by the
/// current log-level (from less verbose to more verbose): none, warn, info,
/// debug, trace.
#[derive(PartialOrd, Ord, PartialEq, Eq)]
pub enum LogLevel {
    None,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Current log-level.
static LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::None as u8);

lazy_static! {
    /// Trace buffer.  An optimization to avoid performance hits due to write
    /// syscalls (context switching).  Each trace macro writes to the trace
    /// buffer.  Call `flush_trace` to print the contents when the program is
    /// done.
    pub static ref TRACE_BUF: Mutex<String> = {
        let s = String::new();
        Mutex::new(s)
    };
}

/// Parse and set the desired log-level.
pub fn init(log_level: &str) -> Result<()> {
    // Parse the log-level.
    match log_level {
        "none"  => set_level_none(),
        "warn"  => set_level_warn(),
        "info"  => set_level_info(),
        "debug" => set_level_debug(),
        "trace" => set_level_trace(),

        _ => return Err(Error::IoError(format!(
            "Unexpected log-level: `{}`", log_level))),
    }

    Ok(())
}

pub fn set_level_none() {
    LOG_LEVEL.store(0, Ordering::SeqCst);
}

pub fn set_level_warn() {
    LOG_LEVEL.store(1, Ordering::SeqCst);
}

pub fn set_level_info() {
    LOG_LEVEL.store(2, Ordering::SeqCst);
}

pub fn set_level_debug() {
    LOG_LEVEL.store(3, Ordering::SeqCst);
}

pub fn set_level_trace() {
    LOG_LEVEL.store(4, Ordering::SeqCst);
}

pub fn get_level() -> LogLevel {
    match LOG_LEVEL.load(Ordering::SeqCst) {
        0 => LogLevel::None,
        1 => LogLevel::Warn,
        2 => LogLevel::Info,
        3 => LogLevel::Debug,
        4 => LogLevel::Trace,

        // This shouldn't happen if the API is defined correctly.
        level => panic!("Unexpected log-level: `{}`", level),
    }
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        if crate::logger::get_level() >= crate::logger::LogLevel::Warn {
            // Separate scope to release the lock.
            {
                use std::fmt::Write;
                let buf = &mut *crate::logger::TRACE_BUF.lock().unwrap();
                writeln!(buf, " WARN: {}", format_args!($($arg)*)).unwrap();
            }

            // Print immediately if the current log-level is not `Trace`.
            if crate::logger::get_level() != crate::logger::LogLevel::Trace {
                crate::logger::flush_trace();
            }
        }
    }};
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        if crate::logger::get_level() >= crate::logger::LogLevel::Info {
            // Separate scope to release the lock.
            {
                use std::fmt::Write;
                let buf = &mut *crate::logger::TRACE_BUF.lock().unwrap();
                writeln!(buf, " INFO: {}", format_args!($($arg)*)).unwrap();
            }

            // Print immediately if the current log-level is not `Trace`.
            if crate::logger::get_level() != crate::logger::LogLevel::Trace {
                crate::logger::flush_trace();
            }
        }
    }};
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {{
        if crate::logger::get_level() >= crate::logger::LogLevel::Debug {
            // Separate scope to release the lock.
            {
                use std::fmt::Write;
                let buf = &mut *crate::logger::TRACE_BUF.lock().unwrap();
                writeln!(buf, "DEBUG: {}", format_args!($($arg)*)).unwrap();
            }

            // Print immediately if the current log-level is not `Trace`.
            if crate::logger::get_level() != crate::logger::LogLevel::Trace {
                crate::logger::flush_trace();
            }
        }
    }};
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {{
        if crate::logger::get_level() >= crate::logger::LogLevel::Trace {
            // Separate scope to release the lock.
            {
                use std::fmt::Write;
                let buf = &mut *crate::logger::TRACE_BUF.lock().unwrap();
                writeln!(buf, "TRACE: {}", format_args!($($arg)*)).unwrap()
            }

            // Do not print immediately here since that's the whole point of
            // having the trace buffer.
        }
    }};
}

/// Flush trace buffer.
pub fn flush_trace() {
    let buf = &mut *crate::logger::TRACE_BUF.lock().unwrap();
    print!("{}", buf);
    *buf = String::new();
}