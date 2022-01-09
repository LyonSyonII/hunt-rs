use std::{io::Write, path::PathBuf};

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

fn append_var(var: &str, txt: &PathBuf) {
    let val = std::env::var(var).unwrap() + &txt.to_string_lossy() + "\n";
    std::env::set_var(var, val);
}

fn search_dir(entry: std::fs::DirEntry, name: &String, first: bool, exact: bool) {
    use rayon::prelude::*;

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
            || path == std::env::current_dir().unwrap()
            || path == dirs::home_dir().unwrap()
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
    use rayon::prelude::*;

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
        std::env::current_dir().unwrap(),
        dirs::home_dir().unwrap(),
        PathBuf::from("/"),
    ];

    for dir in dirs {
        search(&dir, &cli.name, cli.first, cli.exact);
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
