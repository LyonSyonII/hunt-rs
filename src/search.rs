use crate::{searchresult::SearchResult, structs::{Buffers, FileType, Output, Search}};
use rayon::iter::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use std::path::Path;

type Receiver = crossbeam_channel::Receiver<SearchResult>;
type Sender = crossbeam_channel::Sender<SearchResult>;

impl Search {
    pub fn search(&self) -> Buffers {
        let (sender, receiver) = crossbeam_channel::bounded(8);

        // If no limit, search current directory
        if !self.limit {
            let path = if self.canonicalize {
                std::env::current_dir().expect("Could not read current directory")
            } else {
                std::path::Path::new(".").to_owned()
            };
            return rayon::scope(|s| {
                s.spawn(|_| search_path(&path, self, sender));
                receive_paths(receiver, self)
            });
        }
        // Check if paths are valid and canonicalize if necessary
        let dirs = self.dirs.iter().map(|path| {
            if !path.exists() {
                eprintln!("Error: The {:?} directory does not exist", path);
                std::process::exit(1)
            }

            if self.canonicalize {
                std::borrow::Cow::Owned(path.canonicalize().unwrap_or_else(|_| {
                    eprintln!("Error: The {:?} directory does not exist", path);
                    std::process::exit(1)
                }))
            } else {
                std::borrow::Cow::Borrowed(path)
            }
        });

        // Search in directories
        rayon::scope(move |s| {
            s.spawn(move |_| {
                dirs.collect::<Vec<_>>()
                    .into_par_iter()
                    .for_each(|dir| search_path(dir.as_ref(), self, sender.clone()))
            });
            receive_paths(receiver, self)
        })
    }
}

fn receive_paths(receiver: Receiver, search: &Search) -> Buffers {
    use std::io::Write;

    // -f
    if search.first {
        let Ok(path) = receiver.recv() else {
            if search.output == Output::Normal {
                println!("File not found");
            }
            std::process::exit(0)
        };
        println!("{path}");
        std::process::exit(0)
    }

    let stdout = std::io::stdout();
    let mut stdout = std::io::BufWriter::new(stdout.lock());

    // -ss
    if search.output == Output::SuperSimple {
        while let Ok(path) = receiver.recv() {
            writeln!(stdout, "{path}").unwrap();
        }
        stdout.flush().unwrap();
        std::process::exit(0)
    }

    let mut exact = Vec::with_capacity(8);
    let mut contains = Vec::with_capacity(8);
    while let Ok(path) = receiver.recv() {
        match path {
            SearchResult::Contains(path) => contains.push(path),
            SearchResult::Exact(path) => exact.push(path),
        }
    }
    (exact, contains)
}

fn search_path(dir: &Path, search: &Search, sender: Sender) {
    if let Ok(read) = std::fs::read_dir(dir) {
        read.flatten()
            .collect::<Vec<_>>()
            .into_par_iter()
            .for_each(|entry| search_dir(entry, search, sender.clone()));
    } else if search.verbose {
        eprintln!("Could not read {:?}", dir);
    }
}

// pub static MAX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn search_dir(entry: std::fs::DirEntry, search: &Search, sender: Sender) {
    // MAX.fetch_max(backtrace::Backtrace::new().frames().len(), std::sync::atomic::Ordering::Relaxed);

    // Get entry name
    let fname = entry.file_name();
    let fname = fname.to_string_lossy();
    let sname: std::borrow::Cow<str> = if search.case_sensitive {
        fname.as_ref().into()
    } else {
        fname.to_ascii_lowercase().into()
    };
    let path = entry.path();

    if search.explicit_ignore.binary_search(&path).is_ok() {
        return;
    }

    let hardcoded = || {
        search
            .hardcoded_ignore
            .binary_search_by(|p| std::path::Path::new(p).cmp(&path))
            .is_ok()
    };

    if !search.hidden && (is_hidden(&entry) || hardcoded()) {
        return;
    }

    // Read type of file and check if it should be added to search results
    let is_dir = matches!(entry.file_type(), Ok(ftype) if ftype.is_dir());
    let ftype = match search.ftype {
        FileType::Dir => is_dir,
        FileType::File => !is_dir,
        FileType::All => true,
    };

    let starts = search.starts.is_empty() || sname.starts_with(&search.starts);
    let ends = search.ends.is_empty() || sname.ends_with(&search.ends);

    if starts && ends && ftype {
        // If file name is equal to search name, write it to the "Exact" buffer
        if sname == search.name {
            sender.send(SearchResult::exact(path.to_string_lossy().into_owned())).unwrap();
        }
        // If file name contains the search name, write it to the "Contains" buffer
        else if !search.exact && sname.contains(&search.name) {
            let s = if search.output == Output::Normal {
                crate::print::format_with_highlight(&fname, &sname, &path, search)
            } else {
                path.to_string_lossy().into_owned()
            };
            sender.send(SearchResult::contains(s)).unwrap();
        }
    }

    // If entry is not a directory, stop function
    if !is_dir {
        return;
    }

    if let Ok(read) = std::fs::read_dir(&path) {
        read.flatten()
            .collect::<Vec<_>>()
            .into_par_iter()
            .for_each(|entry| search_dir(entry, search, sender.clone()));
    } else if search.verbose {
        eprintln!("Could not read {:?}", path);
    }
}

/// from https://github.com/BurntSushi/ripgrep/blob/master/crates/ignore/src/pathutil.rs
/// 
/// Returns true if and only if this entry is considered to be hidden.
///
/// This only returns true if the base name of the path starts with a `.`.
///
/// On Unix, this implements a more optimized check.
#[cfg(unix)]
pub(crate) fn is_hidden(entry: &std::fs::DirEntry) -> bool {
    use std::os::unix::ffi::OsStrExt;

    file_name(&entry.path()).is_some_and(|name| name.as_bytes().first() == Some(&b'.'))
}

/// from https://github.com/BurntSushi/ripgrep/blob/master/crates/ignore/src/pathutil.rs
/// 
/// Returns true if and only if this entry is considered to be hidden.
///
/// On Windows, this returns true if one of the following is true:
///
/// * The base name of the path starts with a `.`.
/// * The file attributes have the `HIDDEN` property set.
#[cfg(windows)]
pub(crate) fn is_hidden(entry: &std::fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    use winapi_util::file;

    // This looks like we're doing an extra stat call, but on Windows, the
    // directory traverser reuses the metadata retrieved from each directory
    // entry and stores it on the DirEntry itself. So this is "free."
    if let Ok(md) = entry.metadata() {
        if file::is_hidden(md.file_attributes() as u64) {
            return true;
        }
    }
    if let Some(name) = file_name(&entry.path()) {
        name.to_str().map(|s| s.starts_with(".")).unwrap_or(false)
    } else {
        false
    }
}

/// from https://github.com/BurntSushi/ripgrep/blob/master/crates/ignore/src/pathutil.rs
/// 
/// The final component of the path, if it is a normal file.
///
/// If the path terminates in ., .., or consists solely of a root of prefix,
/// file_name will return None.
#[cfg(unix)]
pub(crate) fn file_name<P: AsRef<Path> + ?Sized>(path: &P) -> Option<&std::ffi::OsStr> {
    use std::os::unix::ffi::OsStrExt;

    let path = path.as_ref().as_os_str().as_bytes();
    if path.is_empty()
        || path.len() == 1 && path[0] == b'.'
        || path.last() == Some(&b'.')
        || path.len() >= 2 && path[path.len() - 2..] == b".."[..]
    {
        return None;
    }
    let last_slash = memchr::memrchr(b'/', path).map(|i| i + 1).unwrap_or(0);
    Some(std::ffi::OsStr::from_bytes(&path[last_slash..]))
}

/// from https://github.com/BurntSushi/ripgrep/blob/master/crates/ignore/src/pathutil.rs
/// 
/// The final component of the path, if it is a normal file.
///
/// If the path terminates in ., .., or consists solely of a root of prefix,
/// file_name will return None.
#[cfg(not(unix))]
pub(crate) fn file_name<'a, P: AsRef<Path> + ?Sized>(path: &'a P) -> Option<&'a std::ffi::OsStr> {
    path.as_ref().file_name()
}
