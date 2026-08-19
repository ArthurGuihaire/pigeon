// this might be better than the client one? idk macro rules are annoying, im not changing it
#[macro_export]
macro_rules! debug_print {
    ($($arg:tt)*) => {
        if !cfg!(debug_assertions) {
            println!($($arg)*);
        }
    };
}
