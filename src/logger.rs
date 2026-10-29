#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {{
        print!("\x1b[1;34m[info] \x1b[0m");
        println!($($arg)*);
    }};
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {{
        print!("\x1b[1;32m[debug] \x1b[0m");
        println!($($arg)*);
    }};
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        print!("\x1b[1;31m[error] \x1b[0m");
        println!($($arg)*);
    }};
}
