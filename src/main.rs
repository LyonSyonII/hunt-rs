mod print;
mod search;
mod structs;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> std::io::Result<()> {
    let search = structs::Cli::run();
    search.search();
    search.print_results()
}
