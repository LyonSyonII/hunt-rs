use clap::Parser;

use std::path::PathBuf;

pub type Buffer = Vec<PathBuf>;
pub type Buffers = (Buffer, Buffer);
pub struct Search {
    /// If the search must stop when a match is found.
    pub first: bool,
    /// If only exact matches must be accounted for.
    pub exact: bool,
    /// If all paths should be canonicalized.
    pub canonicalize: bool,
    /// If the search is case sensitive.
    pub case_sensitive: bool,
    /// If the search is limited to specific directories.
    pub limit: bool,
    /// If the output must be verbose or not.
    pub verbose: bool,
    /// If hidden directories must be traversed and hidden files counted as matches.
    pub hidden: bool,
    /// Type of the output.
    ///
    /// Simple makes it not to be highlighted and removes the "Exact:" and "Contains:" distinctions.
    ///
    /// In addition, SuperSimple does not sort the results.
    pub output: Output,
    /// Name of the file/folder we're searching.
    pub name: String,
    /// Pattern the query must start with.
    pub starts: String,
    /// Pattern the query must end with.
    pub ends: String,
    /// Type of the query. It can be a File, a Directory or All.
    pub ftype: FileType,
    /// Directory the user is currently in, used by default to search into.
    pub current_dir: PathBuf,
    /// Directories the user has stated to ignore.
    pub explicit_ignore: Vec<PathBuf>,
    /// Directories hard-coded to be ignored.
    pub hardcoded_ignore: [&'static str; 19],
    /// Directories specified by the user to be searched in.
    pub dirs: Vec<PathBuf>,
}

impl Search {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        first: bool,
        exact: bool,
        canonicalize: bool,
        case_sensitive: bool,
        limit: bool,
        verbose: bool,
        hidden: bool,
        output: u8,
        name: String,
        starts: String,
        ends: String,
        ftype: FileType,
        explicit_ignore: Vec<PathBuf>,
        search_in_dirs: Vec<PathBuf>,
    ) -> Search {
        let output = match output {
            0 => Output::Normal,
            1 => Output::Simple,
            _ => Output::SuperSimple,
        };

        Search {
            first,
            exact,
            canonicalize,
            case_sensitive,
            limit,
            verbose,
            hidden,
            output,
            name,
            starts,
            ends,
            ftype,
            current_dir: std::env::current_dir().expect("Current directory could not be read"),
            explicit_ignore,
            hardcoded_ignore: sorted([
                "/proc",
                "/root",
                "/boot",
                "/dev",
                "/lib",
                "/lib64",
                "/lost+found",
                "/run",
                "/sbin",
                "/sys",
                "/tmp",
                "/var/tmp",
                "/var/lib",
                "/var/log",
                "/var/db",
                "/var/cache",
                "/etc/pacman.d",
                "/etc/sudoers.d",
                "/etc/audit",
            ]),
            dirs: search_in_dirs,
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum Output {
    Normal,
    Simple,
    SuperSimple,
}

pub enum FileType {
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

#[derive(clap::Parser, Debug)]
#[command(
    name = "Hunt",
    about = "Simple command to search a file/folder by name on the current directory.\nBy default it searches all occurrences.",
    version
)]
pub struct Cli {
    /// Stop when first occurrence is found
    #[arg(short, long)]
    first: bool,

    /// Only search for exactly matching occurrences, any file only containing the query will be skipped
    ///
    /// e.g. if query is "SomeFile", "I'mSomeFile" will be skipped, as its name contains more letters than the search
    #[arg(short, long)]
    exact: bool,

    /// If enabled, all paths will be canonicalized.
    #[arg(short, long)]
    canonicalize: bool,

    /// If enabled, the search will be case-sensitive
    ///
    /// Note that case-sensitivity will be activated automatically when the search query contains an uppercase letter
    #[arg(short = 'C', long)]
    case_sensitive: bool,

    /// Print verbose output
    ///
    /// It'll show all errors found:    
    /// e.g. "Could not read /proc/81261/map_files"
    #[arg(short, long)]
    verbose: bool,

    /// Prints without formatting (without "Contains:" and "Exact:")
    ///
    /// -ss Output is not sorted
    #[arg(short, long, action = clap::ArgAction::Count)]
    simple: u8,

    /// If enabled, it searches inside hidden directories
    ///
    /// If not enabled, hidden directories (starting with '.') and "/proc", "/root", "/boot", "/dev", "/lib", "/lib64", "/lost+found", "/run", "/sbin", "/sys", "/tmp", "/var/tmp", "/var/lib", "/var/log", "/var/db", "/var/cache", "/etc/pacman.d", "/etc/sudoers.d" and "/etc/audit" will be skipped
    #[arg(short = 'H', long)]
    hidden: bool,

    /// Only files that start with this will be found
    #[arg(short = 'S', long = "starts")]
    starts_with: Option<String>,

    /// Only files that end with this will be found
    #[arg(short = 'E', long = "ends")]
    ends_with: Option<String>,

    /// Specifies the type of the file
    ///
    /// 'f' -> file | 'd' -> directory
    #[arg(short = 't', long = "type")]
    file_type: Option<String>,

    /// Ignores this directories. The format is:
    ///
    /// -i dir1,dir2,dir3,...
    #[arg(short = 'i', long = "ignore", value_delimiter = ',')]
    ignore_dirs: Option<Vec<PathBuf>>,

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
    #[arg(required = false)]
    search_in_dirs: Vec<PathBuf>,
}

impl Cli {
    pub fn run() -> Search {
        let cli = Self::parse();

        let mut search_in_dirs = cli.search_in_dirs;
        let mut starts = cli.starts_with.unwrap_or_default();
        let mut ends = cli.ends_with.unwrap_or_default();
        let ftype = cli.file_type.into();

        let name = match cli.name {
            // If directory is given but no file name is specified, print files in that directory
            // ex. hunt /home/user
            Some(n) if n == "." || n.contains(std::path::MAIN_SEPARATOR) => {
                search_in_dirs.insert(0, PathBuf::from(n));
                String::new()
            }
            Some(n) => n,
            None => String::new(),
        };

        let case_sensitive =
            cli.case_sensitive || name.contains(|c: char| c.is_alphabetic() && c.is_uppercase());
        if !case_sensitive {
            starts.make_ascii_lowercase();
            ends.make_ascii_lowercase();
        }

        let mut ignore_dirs = cli.ignore_dirs.unwrap_or_default();
        for p in ignore_dirs.iter_mut() {
            if let Ok(c) = p.canonicalize() {
                *p = c;
            }
        }

        Search::new(
            cli.first,
            cli.exact,
            cli.canonicalize,
            case_sensitive,
            !search_in_dirs.is_empty(),
            cli.verbose,
            cli.hidden,
            cli.simple,
            name,
            starts,
            ends,
            ftype,
            ignore_dirs,
            search_in_dirs,
        )
    }
}

const fn sorted<const N: usize>(mut array: [&'static str; N]) -> [&'static str; N] {
    let mut i = 0;
    while i < array.len() {
        let mut j = 0;
        while j < array.len() {
            if lt(array[i], array[j]) {
                let tmp = array[i];
                array[i] = array[j];
                array[j] = tmp;
            }

            j += 1;
        }

        i += 1;
    }
    array
}

const fn lt(lhs: &str, rhs: &str) -> bool {
    let lhs = lhs.as_bytes();
    let rhs = rhs.as_bytes();

    let smallest = if lhs.len() < rhs.len() {
        lhs.len()
    } else {
        rhs.len()
    };

    let mut i = 0;
    while i < smallest {
        let lhs = lhs[i];
        let rhs = rhs[i];

        if lhs < rhs {
            return true;
        }
        if lhs > rhs {
            return false;
        }

        i += 1;
    }
    lhs.len() == smallest
}

#[test]
fn const_sorted() {
    fn check<const N: usize>(mut array: [&'static str; N]) {
        let sorted = sorted(array);
        array.sort_unstable();
        assert_eq!(sorted, array)
    }

    check(["b", "c", "a", "d"]);
    check(["b", "c", "a", "d", "aa"]);
    check([
        "/proc",
        "/root",
        "/booty",
        "/boot",
        "/dev",
        "/lib",
        "/lib64",
        "/lost+found",
        "/run",
        "/sbin",
        "/sys",
        "/tmp",
        "/var/tmp",
        "/var/lib",
        "/var/log",
        "/var/db",
        "/var/cache",
        "/etc/pacman.d",
        "/etc/sudoers.d",
        "/etc/audit",
    ])
}
