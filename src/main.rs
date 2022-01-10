use clap::Parser;
use rayon::iter::{IntoParallelRefIterator, ParallelBridge, ParallelIterator};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(
    name = "Hunt",
    about = "Simple command to search a file/folder by name on the entire drive.\nBy default it searches all occurrences on the system."
)]
struct Cli {
    /// Stop when first occurrence is found
    #[clap(short, long)]
    first: bool,

    /// Only search for exactly matching occurrences
    #[clap(short, long)]
    exact: bool,

    /// Name of the file/folder to search
    name: String,

    /// Directories where you want to search. If provided, hunt will only search there.
    #[clap(required = false)]
    limit_to_dirs: Vec<String>,
}

const EXACT: &str = "SEARCH_EXACT";
const CONTAINS: &str = "SEARCH_CONT";
lazy_static::lazy_static! {
    static ref CURRENT_DIR: PathBuf = std::env::current_dir().expect("Current dir could not be read");
    static ref HOME_DIR: PathBuf = dirs::home_dir().expect("Home dir could not be read");
    static ref ROOT_DIR: PathBuf = PathBuf::from("/");
}

fn append_var(var: &str, txt: &PathBuf) {
    let val = std::env::var(var).unwrap() + &txt.to_string_lossy() + "\n";
    std::env::set_var(var, val);
}

fn search_dir(entry: std::fs::DirEntry, name: &String, first: bool, exact: bool, limit: bool) {
    // Get entry name
    let n = entry.file_name();
    let n = n.to_string_lossy();
    let path = entry.path();

    if n == *name {
        if first {
            println!("{}\n", path.to_string_lossy());
            std::process::exit(0)
        } else {
            append_var(EXACT, &path)
        }
    }
    // If name contains search, print it
    else if !exact && n.contains(name) {
        append_var(CONTAINS, &path)
    }

    // If entry is directory, search inside it
    if let Ok(ftype) = entry.file_type() {
        if !ftype.is_dir() || ((path == *CURRENT_DIR || path == *HOME_DIR) && !limit) {
            return;
        }

        if let Ok(read) = std::fs::read_dir(path) {
            read.par_bridge().for_each(|entry| {
                if let Ok(e) = entry {
                    search_dir(e, &name, first, exact, limit);
                }
            })
        }
    }
}

fn search(dir: &PathBuf, name: &String, first: bool, exact: bool, limit: bool) {
    if let Ok(read) = std::fs::read_dir(dir) {
        read.par_bridge().for_each(|entry| {
            if let Ok(e) = entry {
                search_dir(e, &name, first, exact, limit);
            }
        })
    }
}

fn main() {
    std::env::set_var(EXACT, "");
    std::env::set_var(CONTAINS, "");

    let cli = Cli::parse();

    if cli.limit_to_dirs.is_empty() {
        let dirs: std::collections::HashSet<&PathBuf> =
            std::collections::HashSet::from([&*CURRENT_DIR, &*HOME_DIR, &*ROOT_DIR]);

        for dir in dirs {
            search(dir, &cli.name, cli.first, cli.exact, false);
        }
    } else {
        cli.limit_to_dirs.par_iter().for_each(|dir| {
            search(&PathBuf::from(dir), &cli.name, cli.first, cli.exact, true);
        });
    }

    let ex = std::env::var(EXACT).unwrap();
    let co = std::env::var(CONTAINS).unwrap();

    if ex.is_empty() && co.is_empty() {
        println!("File not found.");
    } else {
        if !cli.exact {
            println!("Contains:\n{}", co);
            println!("Exact:");
        }

        println!("{}", ex);
    }
}
