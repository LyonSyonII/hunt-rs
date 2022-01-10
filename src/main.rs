use std::{path::PathBuf};
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(
    name = "Hunt",
    about = "Simple command to search a file/folder by name on the entire drive.\nBy default it searches all occurrences on the system."
)]
struct Cli {
    /// Name of the file/folder to search
    name: String,

    /// Stop when first occurrence is found
    #[clap(short, long)]
    first: bool,

    /// Only search for exactly matching occurrences
    #[clap(short, long)]
    exact: bool,
}

const EXACT: &str = "SEARCH_EXACT";
const CONTAINS: &str = "SEARCH_CONT";
lazy_static::lazy_static! {
    static ref CURRENT_DIR: PathBuf = std::env::current_dir().expect("Current dir could not be read");
    static ref HOME_DIR: PathBuf = dirs::home_dir().expect("Home dir could not be read");

}


fn append_var(var: &str, txt: &PathBuf) {
    let val = std::env::var(var).unwrap() + &txt.to_string_lossy() + "\n";
    std::env::set_var(var, val);
}

fn search_dir(entry: std::fs::DirEntry, name: &String, first: bool, exact: bool) {
    use rayon::iter::{ ParallelIterator, ParallelBridge };

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
        if !ftype.is_dir()
            || path == *CURRENT_DIR
            || path == *HOME_DIR
        {
            return;
        }

        if let Ok(read) = std::fs::read_dir(path) {
            read.par_bridge().for_each(|entry| {
                if let Ok(e) = entry {
                    search_dir(e, &name, first, exact);
                }
            })
        }
    }
}

fn search(dir: &PathBuf, name: &String, first: bool, exact: bool) {
    use rayon::iter::{ ParallelIterator, ParallelBridge };

    if let Ok(read) = std::fs::read_dir(dir) {
        read.par_bridge().for_each(|entry| {
            if let Ok(e) = entry {
                search_dir(e, &name, first, exact);
            }
        })
    }
}

fn main() {
    std::env::set_var(EXACT, "");
    std::env::set_var(CONTAINS, "");

    let cli = Cli::parse();
    let dirs = [
        &*CURRENT_DIR,
        &*HOME_DIR,
        &PathBuf::from("/"),
    ];

    for dir in dirs {
        search(dir, &cli.name, cli.first, cli.exact);
    }

    let ex = std::env::var(EXACT).unwrap();
    let co = std::env::var(CONTAINS).unwrap();

    if ex == "" && co == "" {
        println!("File not found.");
    } else {
        if !cli.exact {
            println!("Contains:\n{}", co);
            println!("Exact:");
        }

        println!("{}", ex);
    }
}
