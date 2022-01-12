use clap::Parser;
use parking_lot::Mutex;
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::path::{Path, PathBuf};
enum FileType {
    Dir,
    File,
    All,
}

impl From<Option<String>> for FileType {
    fn from(s: Option<String>) -> Self {
        if let Some(s) = s {
            match s.as_str() {
                "d" => FileType::Dir,
                "f" => FileType::File,
                _ => {
                    eprintln!("File type {} not recognized\nPlease use 'f' for files and 'd' for directories\nSee --help for more information\n", s);
                    std::process::exit(1)
                }
            }
        } else {
            FileType::All
        }
    }
}

#[derive(Parser, Debug)]
#[clap(
    name = "Hunt",
    about = "Simple command to search a file/folder by name on the entire drive\nBy default it searches all occurrences on the system"
)]
struct Cli {
    /// Stop when first occurrence is found
    #[clap(short, long)]
    first: bool,

    /// Only search for exactly matching occurrences
    #[clap(short, long)]
    exact: bool,

    /// Print verbose output
    ///
    /// It'll show all errors found:    
    /// e.g. "Could not read /proc/81261/map_files"
    #[clap(short, long)]
    verbose: bool,

    /// Prints without formatting (without "Contains:" and "Exact:")
    #[clap(short, long)]
    simple: bool,

    /// Only files that start with this will be found
    #[clap(long = "starts")]
    starts_with: Option<String>,

    /// Only files that end with this will be found
    #[clap(long = "ends")]
    ends_with: Option<String>,

    /// Specifies the type of the file
    ///
    /// 'f' -> file | 'd' -> directory
    #[clap(short = 't', long = "type")]
    file_type: Option<String>,

    /// Name of the file/folder to search. If starts/ends are specified, this field can be skipped
    name: Option<String>,

    /// Directories where you want to search
    ///
    /// If provided, hunt will only search there
    ///
    /// These directories are treated independently, so if one is nested into another the search will be done two times:
    ///
    /// e.g. "hunt somefile /home/user /home/user/downloads" will search in the home directory, and because /home/user/downloads is inside it, /downloads will be traversed two times
    #[clap(required = false)]
    limit_to_dirs: Vec<String>,
}

struct Search<'a> {
    name: &'a str,
    starts: &'a str,
    ends: &'a str,
    ftype: &'a FileType,
}

impl Search<'_> {
    fn new<'a>(name: &'a str, starts: &'a str, ends: &'a str, ftype: &'a FileType) -> Search<'a> {
        Search {
            name,
            starts,
            ends,
            ftype,
        }
    }
}

#[derive(Clone, Copy)]
struct Args {
    first: bool,
    exact: bool,
    limit: bool,
    verbose: bool,
}

impl Args {
    fn new(first: bool, exact: bool, limit: bool, verbose: bool) -> Args {
        Args {
            first,
            exact,
            limit,
            verbose,
        }
    }
}

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

lazy_static::lazy_static! {
    static ref CURRENT_DIR: PathBuf = std::env::current_dir().expect("Current dir could not be read");
    static ref HOME_DIR: PathBuf = dirs::home_dir().expect("Home dir could not be read");
    static ref ROOT_DIR: PathBuf = PathBuf::from("/");
    static ref BUFFER: Buffer = Mutex::new((String::new(), String::new()));
}

type Buffer = Mutex<(std::string::String, std::string::String)>;

fn append_var(var: &mut std::string::String, txt: &Path) {
    var.push_str(&txt.to_string_lossy());
    var.push('\n');
}

fn search_dir(entry: std::fs::DirEntry, search: &Search, args: Args) {
    // Get entry name
    let n = entry.file_name();
    let n = n.to_string_lossy();
    let path = entry.path();

    let (name, starts, ends, ftype) = (search.name, search.starts, search.ends, search.ftype);
    let (first, exact, limit, verbose) = (args.first, args.exact, args.limit, args.verbose);

    // Read type of file and check if it should be added to search results
    let ftype = match ftype {
        FileType::Dir => {
            if let Ok(ftype) = entry.file_type() {
                ftype.is_dir()
            } else {
                false
            }
        }
        FileType::File => {
            if let Ok(ftype) = entry.file_type() {
                ftype.is_file()
            } else {
                false
            }
        }
        FileType::All => true,
    };

    // If match is exact, print it
    if ftype && n == *name && n.starts_with(starts) && n.ends_with(ends) {
        if first {
            println!("{}\n", path.to_string_lossy());
            std::process::exit(0)
        } else {
            append_var(&mut BUFFER.lock().0, &path)
        }
    }
    // If name contains search, print it
    else if !exact && ftype && n.contains(name) && n.starts_with(starts) && n.ends_with(ends) {
        if first {
            println!("{}\n", path.to_string_lossy());
            std::process::exit(0)
        } else {
            append_var(&mut BUFFER.lock().1, &path)
        }
    }

    // If entry is directory, search inside it
    if let Ok(ftype) = entry.file_type() {
        if !ftype.is_dir() || ((path == *CURRENT_DIR || path == *HOME_DIR) && !limit) {
            return;
        }

        if let Ok(read) = std::fs::read_dir(&path) {
            read.par_bridge().for_each(|entry| {
                if let Ok(e) = entry {
                    search_dir(e, search, args);
                }
            })
        } else if verbose {
            eprintln!("Could not read {:?}", path);
        }
    } else if verbose {
        eprintln!("Could not get file type for {:?}", entry);
    }
}

fn search_path(dir: &std::path::Path, search: Search, args: Args) {
    if let Ok(read) = std::fs::read_dir(dir) {
        read.par_bridge().for_each(|entry| {
            if let Ok(e) = entry {
                search_dir(e, &search, args);
            }
        })
    } else if args.verbose {
        eprintln!("Could not read {:?}", dir);
    }
}

fn main() {
    use indexmap::IndexSet;

    let mut cli = Cli::parse();

    let starts = cli.starts_with.unwrap_or_default();
    let ends = cli.ends_with.unwrap_or_default();
    let ftype = cli.file_type.into();

    let name = if let Some(n) = cli.name {
        if n == "." || n.contains('/') {
            cli.limit_to_dirs.insert(0, n);
            String::new()
        } else {
            n
        }
    } else {
        String::new()
    };

    if cli.limit_to_dirs.is_empty() {
        let dirs = IndexSet::from([&*CURRENT_DIR, &*HOME_DIR, &*ROOT_DIR]).into_iter();

        // If only search for first, do it in order (less expensive to more)
        if cli.first {
            for dir in dirs {
                search_path(
                    dir,
                    Search::new(&name, &starts, &ends, &ftype),
                    Args::new(true, cli.exact, false, cli.verbose),
                );
            }
        }
        // If search all occurrences, multithread search
        else {
            dirs.par_bridge().for_each(|dir| {
                search_path(
                    dir,
                    Search::new(&name, &starts, &ends, &ftype),
                    Args::new(false, cli.exact, false, cli.verbose),
                );
            });
        }
    } else {
        // Check if paths are valid
        let dirs = cli.limit_to_dirs.iter().map(|s| {
            std::fs::canonicalize(s.as_str()).unwrap_or_else(|_| {
                eprintln!("ERROR: The {:?} directory does not exist", s);
                std::process::exit(1);
            })
        });
        // Remove duplicates
        let dirs = IndexSet::<PathBuf>::from_iter(dirs).into_iter();

        // Search in directories
        dirs.par_bridge().for_each(|dir| {
            search_path(
                &dir,
                Search::new(&name, &starts, &ends, &ftype),
                Args::new(cli.first, cli.exact, true, cli.verbose),
            );
        });
    };

    let (ex, co) = &*BUFFER.lock();

    if cli.simple {
        print!("{}{}", co, ex);
        return;
    }

    if ex.is_empty() && co.is_empty() {
        println!("File not found\n");
    } else {
        if !cli.exact {
            println!("Contains:\n{}", co);
            println!("Exact:");
        }

        println!("{}", ex);
    }
}
