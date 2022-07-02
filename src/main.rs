mod print;
mod search;
mod structs;
use structs::Cli;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> std::io::Result<()> {
    let search = Cli::run();
    search.search();
    search.print_results()
}
