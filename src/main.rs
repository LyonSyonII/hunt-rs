mod print;
mod search;
mod structs;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> std::io::Result<()> {
    let search = structs::Cli::run();
    std::env::set_var("RUST_MIN_STACK", format!("{}", 1024 * 1024 * 1024));
    search.search();
    search.print_results()
}
