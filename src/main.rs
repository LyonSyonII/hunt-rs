// TODO: Upload updated package

use clap::Parser;
use parking_lot::Mutex;
use rayon::{ iter::{ParallelBridge, ParallelIterator}, slice::ParallelSliceMut, str::ParallelString };
use std::{path::{Path, PathBuf}};

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

    /// Only search for exactly matching occurrences, any file only containing the query will be skipped 
    /// 
    /// e.g. if query is "SomeFile", "I'mSomeFile" will be skipped, as its name contains more letters than the query. 
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
    
    /// If enabled, it searches inside hidden directories
    /// 
    /// If not enabled, hidden directories (starting with '.'), "/proc", "/root", "/boot", "/dev", "/lib", "/lib64", "/lost+found", "/run", "/sbin", "/sys", "/tmp", "/var/tmp", "/var/lib", "/var/log", "/var/db", "/var/cache", "/etc/pacman.d", "/etc/sudoers.d" and "/etc/audit" will be skipped
    #[clap(short, long)]
    hidden: bool,

    /// Only files that start with this will be found
    #[clap(short = 'S', long = "starts")]
    starts_with: Option<String>,

    /// Only files that end with this will be found
    #[clap(short = 'E', long = "ends")]
    ends_with: Option<String>,

    /// Specifies the type of the file
    ///
    /// 'f' -> file | 'd' -> directory
    #[clap(short = 't', long = "type")]
    file_type: Option<String>,
    
    /// Ignores this directories. The format is:
    /// 
    /// -i dir1,dir2,dir3,...
    #[clap(short = 'i', long = "ignore", parse(try_from_str = parse_ignore_dirs))]
    ignore_dirs: Option<std::collections::HashSet<PathBuf>>,

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

fn parse_ignore_dirs(inp: &str) -> Result<std::collections::HashSet<PathBuf>, std::string::String> {
    let inp = inp.trim().replace(' ', "");
    Ok(std::collections::HashSet::from_iter(inp.split(',').map(|s| s.into())))
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

struct Args<'a> {
    first: bool,
    exact: bool,
    limit: bool,
    verbose: bool,
    hidden: bool,
    ignore: &'a Option<std::collections::HashSet<PathBuf>>,
    case_sensitive: bool,
}

impl Args<'_> {
    fn new(first: bool, exact: bool, limit: bool, verbose: bool, hidden: bool, ignore: &Option<std::collections::HashSet<PathBuf>>, case_sensitive: bool) -> Args {
        Args {
            first,
            exact,
            limit,
            verbose,
            hidden,
            ignore,
            case_sensitive,
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
    static ref IGNORE_PATHS: std::collections::HashSet<PathBuf> = std::collections::HashSet::from_iter(["/proc", "/root", "/boot", "/dev", "/lib", "/lib64", "/lost+found", "/run", "/sbin", "/sys", "/tmp", "/var/tmp", "/var/lib", "/var/log", "/var/db", "/var/cache", "/etc/pacman.d", "/etc/sudoers.d", "/etc/audit"].iter().map(|p| p.into()));
    static ref FOUND: Mutex<bool> = Mutex::new(false);
}

type Buffer = Mutex<(String, String)>;

fn append_var(var: &mut String, txt: &Path) {
    var.push_str(&txt.to_string_lossy());
    var.push('\n');
}

fn search_dir(entry: std::fs::DirEntry, search: &Search, args: &Args) {
    // Get entry name
    let n = entry.file_name();
    let n = match args.case_sensitive { 
        true => n.to_string_lossy().into(),
        false => n.to_string_lossy().to_ascii_lowercase()
    };

    let path = entry.path();
    let path = path.as_path();

    let (name, starts, ends, ftype) = (search.name, search.starts, search.ends, search.ftype);
    let (first, exact, limit, verbose, hidden, ignore) = (args.first, args.exact, args.limit, args.verbose, args.hidden, args.ignore);
    
    if !hidden && (n.starts_with('.') || IGNORE_PATHS.contains(path)) {
        return;
    }

    if let Some(ignore) = ignore {
        if ignore.contains(path) {
            return;
        }
    }

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
            let mut found = FOUND.lock();
            if *found { return; }

            *found = true;
            println!("{}\n", path.to_string_lossy());
            std::process::exit(0)
        } else {
            append_var(&mut BUFFER.lock().0, path)
        }
    }
    // If name contains search, print it
    else if !exact && ftype && n.contains(name) && n.starts_with(starts) && n.ends_with(ends) {
        if first {
            let mut found = FOUND.lock();
            if *found { return; }

            *found = true;
            println!("{}\n", path.to_string_lossy());
            std::process::exit(0)
        } else {
            append_var(&mut BUFFER.lock().1, path)
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
                search_dir(e, &search, &args);
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

    let c_sensitive = name.contains(|c: char| c.is_alphabetic() && c.is_uppercase());
    
    if cli.limit_to_dirs.is_empty() {
        let dirs = IndexSet::from([&*CURRENT_DIR, &*HOME_DIR, &*ROOT_DIR]).into_iter();
        
        // If only search for first, do it in order (less expensive to more)
        if cli.first {
            for dir in dirs {
                search_path(
                    dir,
                    Search::new(&name, &starts, &ends, &ftype),
                    Args::new(true, cli.exact, false, cli.verbose, cli.hidden, &cli.ignore_dirs, c_sensitive),
                );
            }
        }
        // If search all occurrences, multithread search
        else {
            dirs.par_bridge().for_each(|dir| {
                search_path(
                    dir,
                    Search::new(&name, &starts, &ends, &ftype),
                    Args::new(false, cli.exact, false, cli.verbose, cli.hidden, &cli.ignore_dirs, c_sensitive),
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
                Args::new(cli.first, cli.exact, true, cli.verbose, cli.hidden, &cli.ignore_dirs, c_sensitive),
            );
        });
    };
    
    let (ex, co) = &mut *BUFFER.lock();
    let (ex, co) = (sort_results(ex), sort_results(co));

    if cli.simple {
        print!("{}{}", co, ex);
        return;
    }
    
    if ex.is_empty() && co.is_empty() {
        println!("File not found\n");
    } else {
        if !cli.exact {
            println!("Contains:{}", co);
            print!("Exact:");
        }

        println!("{}", ex);
    }
}


// Utility functions

fn sort_results(sort: &mut String) -> String {
    let mut sort = sort.par_split('\n').collect::<Vec<&str>>();
    sort.par_sort_unstable();
    let mut sort = sort.join("\n");
    sort.push('\n');
    sort
}
