mod print;
mod search;
mod structs;
// mod bumpalo_herd;

#[cfg(not(test))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "debug")]
pub(crate) struct Perf {
    time: std::time::Duration,
    ctx: &'static str,
}

#[cfg(feature = "debug")]
impl Drop for Perf {
    fn drop(&mut self) {
        eprintln!("{}: {:?}", self.ctx, self.time);
    }
}

#[macro_export]
macro_rules! perf {
    (ctx = $ctx:expr; $($code:tt)* ) => {
        #[cfg(feature = "debug")]
        let _start = std::time::Instant::now();
        $($code)*
        #[cfg(feature = "debug")]
        let _end = std::time::Instant::now();
        #[cfg(feature = "debug")]
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
    std::env::set_var("RUST_MIN_STACK", format!("{}", 1024 * 1024 * 1024));

    perf! {
        ctx = "search";
        let buffers = search.search();
    }
    search.print_results(buffers)?;
    Ok(())
}
