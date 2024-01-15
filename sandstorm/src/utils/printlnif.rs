/// The same as the `println!` macro, but takes as first parameter a condition on whether to print.
/// If true, the contents will be printed. Otherwise, nothing will happen.
#[macro_export]
macro_rules! printlnif {
    ($condition:expr) => {
        if $condition {
            std::println!();
        }
    };
    ($condition:expr, $($arg:tt)*) => {{
        if $condition {
            std::println!($($arg)*);
        }
    }};
}
