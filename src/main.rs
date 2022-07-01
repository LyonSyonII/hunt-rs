use clap::Parser;
use parking_lot::Mutex;
use colored::Colorize;
use rayon::{
    iter::{ParallelBridge, ParallelIterator, IntoParallelIterator},
    slice::ParallelSliceMut,
};
use std::{
    io::{Write},
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool, ops::Index,
};

#[derive(PartialEq, Clone, Copy)]
enum Output {
    Normal,
    Simple,
    SuperSimple,
}

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

    /// If enabled, the search will be case-sensitive
    /// 
    /// Note that case-sensitivity will be activated automatically when the search query contains an uppercase letter.
    #[clap(short, long)]
    case_sensitive: bool,

    /// Print verbose output
    ///
    /// It'll show all errors found:    
    /// e.g. "Could not read /proc/81261/map_files"
    #[clap(short, long)]
    verbose: bool,

    /// Prints without formatting (without "Contains:" and "Exact:")
    ///
    /// -ss Output is not sorted
    #[clap(short, long, action = clap::ArgAction::Count)]
    simple: u8,

    /// If enabled, it searches inside hidden directories
    ///
    /// If not enabled, hidden directories (starting with '.') and "/proc", "/root", "/boot", "/dev", "/lib", "/lib64", "/lost+found", "/run", "/sbin", "/sys", "/tmp", "/var/tmp", "/var/lib", "/var/log", "/var/db", "/var/cache", "/etc/pacman.d", "/etc/sudoers.d" and "/etc/audit" will be skipped
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
    ignore_dirs: Option<HashSet<PathBuf>>,

    /// Name of the file/folder to search. If starts/ends are specified, this field can be skipped
    name: Option<String>,

    /// Directories where you want to search
    ///
    /// If provided, hunt will only search there
    /// 
    /// If not provided, hunt will search in the current directory
    ///
    /// These directories are treated independently, so if one is nested into another the search will be done two times:
    ///
    /// e.g. "hunt somefile /home/user /home/user/downloads" will search in the home directory, and because /home/user/downloads is inside it, /downloads will be traversed two times
    #[clap(required = false)]
    search_in_dirs: Vec<PathBuf>,
}

fn parse_ignore_dirs(inp: &str) -> Result<HashSet<PathBuf>, String> {
    let inp = inp.trim().replace(',', " ");
    Ok(HashSet::from_iter(inp.split(',').map(PathBuf::from)))
}


struct Search<'a> {
    name: &'a str,
    starts: &'a str,
    ends: &'a str,
    ftype: &'a FileType,
    current_dir: PathBuf,
    hardcoded_ignore: HashSet<&'static Path>
}

impl<'a> Search<'a> {
    fn new(name: &'a str, starts: &'a str, ends: &'a str, ftype: &'a FileType) -> Search<'a> {
        Search { 
            name, 
            starts, 
            ends, 
            ftype, 
            current_dir: std::env::current_dir().expect("Current directory could not be read"), 
            hardcoded_ignore: HashSet::from_iter(["/proc", "/root", "/boot", "/dev", "/lib", "/lib64", "/lost+found", "/run", "/sbin", "/sys", "/tmp", "/var/tmp", "/var/lib", "/var/log", "/var/db", "/var/cache", "/etc/pacman.d", "/etc/sudoers.d", "/etc/audit"].iter().map(Path::new)) 
        }
    }
}

#[allow(clippy::too_many_arguments)]
struct Args<'a> {
    first: bool,
    exact: bool,
    limit: bool,
    verbose: bool,
    hidden: bool,
    ignore: &'a std::collections::HashSet<PathBuf>,
    case_sensitive: bool,
}

impl<'a> Args<'a> {
    fn new(first: bool, exact: bool, limit: bool, verbose: bool, hidden: bool, ignore: &'a HashSet<PathBuf>, case_sensitive: bool) -> Args<'a> {
        Args { first, exact, limit, verbose, hidden, ignore, case_sensitive }
    }
}

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
static FOUND: AtomicBool = AtomicBool::new(false);

type Buffers = (Mutex<Buffer>, Mutex<Buffer>);
type Buffer = Vec<PathBuf>;

fn print_var(var: &mut Buffer, first: bool, path: PathBuf) {
    if first {
        let found = FOUND.load(std::sync::atomic::Ordering::Relaxed);
        if found {
            return;
        }

        FOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        println!("{}", path.display());
        std::process::exit(0)
    } else {
        var.push(path);
    }
}

fn search_dir(entry: std::fs::DirEntry, search: &Search, args: &Args, buffers: &Buffers) {
    // Get entry name
    let fname = if args.case_sensitive {
        entry.file_name()
    } else {
        entry.file_name().to_ascii_lowercase()
    };
    let fname = fname.to_string_lossy();
    let path = entry.path();
    
    if args.ignore.contains(&path) {
        return;
    }

    if !args.hidden && (fname.starts_with('.') || search.hardcoded_ignore.contains(path.as_path())) {
        return;
    }

    // Read type of file and check if it should be added to search results
    let is_dir = matches!(entry.file_type(), Ok(ftype) if ftype.is_dir());
    let ftype = match search.ftype {
        FileType::Dir => {
            is_dir
        }
        FileType::File => {
            !is_dir
        }
        FileType::All => true,
    };

    let starts = !search.starts.is_empty() || fname.starts_with(search.starts);
    let ends = !search.ends.is_empty() || fname.ends_with(search.ends);
    if starts && ends && ftype {
        // If file name is equal to search name, write it to the "Exact" buffer
        if fname == search.name {
            print_var(&mut buffers.0.lock(), args.first, path.clone());
        } 
        // If file name contains the search name, write it to the "Contains" buffer
        else if !args.exact && fname.contains(search.name) {
            print_var(&mut buffers.1.lock(), args.first, path.clone());
        }
    }
    
    // If entry is not a directory, stop function 
    if !is_dir {
        return;
    }
    // If entry is a directory, search inside it
    if let Ok(read) = std::fs::read_dir(&path) {
        read.flatten().par_bridge().for_each(|entry| {
            search_dir(entry, search, args, buffers);
        })
    } else if args.verbose {
        eprintln!("Could not read {:?}", path);
    }
}

fn search_path(dir: &Path, search: &Search, args: &Args, buffers: &Buffers) {
    if let Ok(read) = std::fs::read_dir(dir) {
        read.flatten().par_bridge().for_each(|entry| {
            search_dir(entry, search, args, buffers);
        })
    } else if args.verbose {
        eprintln!("Could not read {:?}", dir);
    }
}

fn print_with_highlight(stdout: &mut std::io::StdoutLock, path: &Path, name: &str, simple: Output, case_sensitive: bool) -> std::io::Result<()> {
    if simple == Output::Normal {
        let path = path.to_string_lossy();
        let search = if case_sensitive {
            path.to_string()
        } else {
            path.to_ascii_lowercase()
        };

        let start = search.rfind(name).unwrap();
        let end = start + name.len();
        return writeln!(stdout, "{}{}{}", path.index(..start), path.index(start..end).bright_red().bold(), path.index(end..));
    } 
        
    writeln!(stdout, "{}", path.display())
}

fn main() -> std::io::Result<()> {
    let mut cli = Cli::parse();

    let starts = cli.starts_with.unwrap_or_default();
    let ends = cli.ends_with.unwrap_or_default();
    let ftype = cli.file_type.into();

    let name = match cli.name {
        // If directory is given but no file name is specified, print files in that directory
        // ex. hunt /home/user
        Some(n) if n == "." || n.contains('/') => {
            cli.search_in_dirs.insert(0, PathBuf::from(n));
            String::new()
        }
        Some(n) => n,
        None => String::new(),
    };

    let simple = match cli.simple {
        0 => Output::Normal,
        1 => Output::Simple,
        2.. => Output::SuperSimple,
    };

    let search = Search::new(&name, &starts, &ends, &ftype);

    let c_sensitive = cli.case_sensitive || name.contains(|c: char| c.is_alphabetic() && c.is_uppercase());
    let ignore_dirs = cli.ignore_dirs.unwrap_or_default();
    let args = Args::new(
        cli.first,
        cli.exact,
        !cli.search_in_dirs.is_empty(),
        cli.verbose,
        cli.hidden,
        &ignore_dirs,
        c_sensitive,
    );

    let buffers: Buffers = (Mutex::new(Vec::new()), Mutex::new(Vec::new()));
    
    // If no limit, search current directory
    if !args.limit {
        search_path(&search.current_dir, &search, &args, &buffers);
    } else {
        // Check if paths are valid
        let dirs = cli.search_in_dirs.iter().map(|s| {
            std::fs::canonicalize(s).unwrap_or_else(|_| {
                eprintln!("ERROR: The {:?} directory does not exist", s);
                std::process::exit(1);
            })
        });
        // Search in directories
        dirs.par_bridge().for_each(|dir| {
            search_path(&dir, &search, &args, &buffers);
        });
    };

    // Get results and sort them
    let (mut ex, mut co) = (buffers.0.into_inner(), buffers.1.into_inner());
    
    if ex.is_empty() && co.is_empty() {
        println!("File not found");
        return Ok(());
    }

    if simple != Output::SuperSimple {
        co.par_sort_unstable();
        ex.par_sort_unstable();
    }
    
    // Print results
    let mut stdout = std::io::stdout().lock();
    
    if simple == Output::Normal {
        writeln!(stdout, "Contains:")?;
    }
    for path in co {
        print_with_highlight(&mut stdout, &path, search.name, simple, args.case_sensitive)?;
    }
    if simple == Output::Normal { 
        writeln!(stdout, "\nExact:")?; 
    }
    for path in ex {
        writeln!(stdout, "{}", path.display())?;
    }
    Ok(())
}