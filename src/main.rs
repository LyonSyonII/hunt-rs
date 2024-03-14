mod print;
mod search;
mod searchresult;
mod structs;

#[cfg(not(any(test, miri)))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "perf")]
pub(crate) struct Perf {
    time: std::time::Duration,
    ctx: &'static str,
}

#[cfg(feature = "perf")]
impl Drop for Perf {
    fn drop(&mut self) {
        eprintln!("{}: {:?}", self.ctx, self.time);
    }
}

#[macro_export]
macro_rules! perf {
    (ctx = $ctx:expr; $($code:tt)* ) => {
        #[cfg(feature = "perf")]
        let _start = std::time::Instant::now();
        $($code)*
        #[cfg(feature = "perf")]
        let _end = std::time::Instant::now();
        #[cfg(feature = "perf")]
        $crate::Perf {
            time: _end - _start,
            ctx: $ctx
        };
    };
    (disable; ctx = $ctx:expr; $($code:tt)* ) => {
        $($code)*
    };
    ($($code:tt)* ) => {
        $crate::perf!(ctx = stringify!( $($code)* ); $($code)*)
    };
}

fn main() -> std::io::Result<()> {
    let search = structs::Cli::run();
    // std::env::set_var("RUST_MIN_STACK", format!("{}", 1024 * 1024 * 1024));

    perf! {
        ctx = "search";
        let buffers = search.search();
    }
    search.print_results(buffers)?;
    // dbg!(&crate::search::MAX);
    Ok(())
}
