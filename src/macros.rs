//! Macros Used Throughout the Library

/// Convenience macro to log messages at provided level and to print them to `stderr`
#[macro_export]
macro_rules! log_and_stderr {
    ($level:ident, $msg:expr) => {
        log::$level!("{}", $msg);
        eprintln!("{}", $msg);
    };
    ($level:ident, $msg:expr, $arg:expr) => {
        log::$level!("{} {}", $msg, $arg);
        eprintln!("{} {}", $msg, $arg);
    };
}

/// Convenience macro to log messages at trace level and to print them to `stderr`
#[macro_export]
macro_rules! trace_and_stderr {
    ($msg:expr) => {
        log::trace!("{}", $msg);
        eprintln!("{}", $msg);
    };
    ($msg:expr, $arg:expr) => {
        log::trace!("{} {}", $msg, $arg);
        eprintln!("{} {}", $msg, $arg);
    };
}

/// Compares against an enum variant without taking the value into account
#[macro_export]
macro_rules! is_enum_variant {
    ($val:ident, $var:path) => {
        match $val {
            $var(..) => true,
            _ => false,
        }
    };
}
