  
#[macro_export]
macro_rules! trace_func {
    ($($arg:tt)*) => {{
        trace!("{}: {}", function!(), format_args!($($arg)*))
    }}
}

#[macro_export]
macro_rules! trace_hexdump {
    ($addr: expr, $sym: expr, $vec: expr) => {{
        for s in crate::hexdump::hexdump($addr, &$vec)
            .split(|c| c == '\n')
        {
            trace_func!("{}: {}", $sym, s);
        }
    }}
}

#[macro_export]
macro_rules! trace_multi {
    ($sym: expr, $val: expr) => {{
        for s in format!("{:#x?}", $val)
            .split(|c| c == '\n')
        {
            trace_func!("{}: {}", $sym, s);
        }
    }}
}