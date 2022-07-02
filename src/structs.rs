use clap::Parser;
use parking_lot::Mutex;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

pub type Buffer = Vec<PathBuf>;
pub type Buffers = (Mutex<Buffer>, Mutex<Buffer>);
pub struct Search {
    /// If the search must stop when a match is found.
    pub first: bool,
    /// If only exact matches must be accounted for.
    pub exact: bool,
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
    pub explicit_ignore: HashSet<PathBuf>,
    /// Directories hard-coded to be ignored.
    pub hardcoded_ignore: HashSet<&'static Path>,
    /// Directories specified by the user to be searched in.
    pub dirs: Vec<PathBuf>,
    pub buffers: Buffers,
}

impl Search {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        first: bool,
        exact: bool,
        case_sensitive: bool,
        limit: bool,
        verbose: bool,
        hidden: bool,
        output: u8,
        name: String,
        starts: String,
        ends: String,
        ftype: FileType,
        explicit_ignore: HashSet<PathBuf>,
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
            hardcoded_ignore: HashSet::from_iter(
                [
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
                ]
                .iter()
                .map(Path::new),
            ),
            dirs: search_in_dirs,
            buffers: (
                parking_lot::Mutex::new(Vec::new()),
                parking_lot::Mutex::new(Vec::new()),
            ),
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
#[clap(
    name = "Hunt",
    about = "Simple command to search a file/folder by name on the entire drive\nBy default it searches all occurrences on the system"
)]
pub struct Cli {
    /// Stop when first occurrence is found
    #[clap(short, long)]
    first: bool,

    /// Only search for exactly matching occurrences, any file only containing the query will be skipped
    ///
    /// e.g. if query is "SomeFile", "I'mSomeFile" will be skipped, as its name contains more letters than the search
    #[clap(short, long)]
    exact: bool,

    /// If enabled, the search will be case-sensitive
    ///
    /// Note that case-sensitivity will be activated automatically when the search query contains an uppercase letter
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

impl Cli {
    pub fn run() -> Search {
        let cli = Self::parse();

        let mut search_in_dirs = cli.search_in_dirs;
        let starts = cli.starts_with.unwrap_or_default();
        let ends = cli.ends_with.unwrap_or_default();
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
        let ignore_dirs = cli.ignore_dirs.unwrap_or_default();

        Search::new(
            cli.first,
            cli.exact,
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

fn parse_ignore_dirs(inp: &str) -> Result<HashSet<PathBuf>, String> {
    let inp = inp.trim().replace(',', " ");
    Ok(HashSet::from_iter(inp.split(',').map(PathBuf::from)))
}
