mod print;
mod search;
mod searchresult;
mod structs;

#[cfg(not(any(test, miri)))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> std::io::Result<()> {
    profi::print_on_exit!(stderr);
    let search = structs::Cli::run();
    
    let buffers = search.search();
    search.print_results(buffers)?;

    Ok(())
}
