use clap::Parser;
use rayon::iter::{ ParallelBridge, ParallelIterator};
use std::path::{ PathBuf };

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
    
    /// Print verbose output. 
    /// 
    /// It'll show all errors found:    
    /// e.g. "Could not read /proc/81261/map_files" 
    #[clap(short, long)]
    verbose: bool,

    /// Name of the file/folder to search
    name: String,

    /// Directories where you want to search. 
    /// 
    /// If provided, hunt will only search there. 
    /// 
    /// These directories are completely different for hunt, so if one is nested into other the search will be done two times:
    /// 
    /// e.g. "hunt somefile / /home" will search in the root directory, and because /home is inside it, /home will be traversed two times.
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

fn search_dir(entry: std::fs::DirEntry, name: &String, first: bool, exact: bool, limit: bool, verbose: bool) {
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

        if let Ok(read) = std::fs::read_dir(&path) {
            read.par_bridge().for_each(|entry| {
                if let Ok(e) = entry {
                    search_dir(e, &name, first, exact, limit, verbose);
                }
            })
        } else if verbose {
            eprintln!("Could not read {:?}", path);
        }
    }
}

fn search(dir: &PathBuf, name: &String, first: bool, exact: bool, limit: bool, verbose: bool) {
    if let Ok(read) = std::fs::read_dir(dir) {
        read.par_bridge().for_each(|entry| {
            if let Ok(e) = entry {
                search_dir(e, &name, first, exact, limit, verbose);
            }
        })
    } else if verbose {
        eprintln!("Could not read {:?}", dir);
    }
}

fn main() {
    use indexmap::IndexSet;

    std::env::set_var(EXACT, "");
    std::env::set_var(CONTAINS, "");

    let cli = Cli::parse();

    if cli.limit_to_dirs.is_empty() {
        let dirs =
            IndexSet::from([&*CURRENT_DIR, &*HOME_DIR, &*ROOT_DIR]).into_iter();
        
        // If only search for first, do it in order (less expensive to more)
        if cli.first {
            for dir in dirs {
                search(dir, &cli.name, true, cli.exact, false, cli.verbose);
            }
        } 
        // If search all occurrences, multithread search
        else {
            dirs.par_bridge().for_each(|dir| {
                search(dir, &cli.name, false, cli.exact, false, cli.verbose);
            });
        }
        
        
    } else {
        // Check if paths are valid
        let dirs = cli.limit_to_dirs.iter().map(|s| {
            PathBuf::from(
                std::fs::canonicalize(s).unwrap_or_else(|_| {
                    eprintln!("ERROR: The {:?} directory does not exist", s);
                    std::process::exit(1);
                })
            )
        });
        // Remove duplicates
        let dirs = IndexSet::<PathBuf>::from_iter(dirs).into_iter();

        // Search in directories
        dirs.par_bridge().for_each(|dir| {
            search(&dir, &cli.name, cli.first, cli.exact, true, cli.verbose);
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
