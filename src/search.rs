use crate::{
    searchresult::SearchResult,
    structs::{Buffers, FileType, Output, Search},
};
use std::path::Path;

type Receiver = crossbeam_channel::Receiver<SearchResult>;
type Sender = crossbeam_channel::Sender<SearchResult>;

impl Search {
    #[profi::profile]
    pub fn search(&self) -> Buffers {
        let (sender, receiver) = crossbeam_channel::bounded(8);

        // If no limit, search current directory
        if !self.limit {
            let path = if self.canonicalize {
                std::borrow::Cow::Owned(
                    std::env::current_dir().expect("Could not read current directory"),
                )
            } else {
                std::borrow::Cow::Borrowed(std::path::Path::new("."))
            };
            return rayon::scope(|s| {
                s.spawn(|_| search_dir(path, self, sender, 0));
                receive_paths(receiver, self)
            });
        }
        // Check if paths are valid and canonicalize if necessary
        let dirs = self.dirs.iter().map(|path| {
            if !path.exists() {
                eprintln!("Error: The {path:?} directory does not exist");
                std::process::exit(1)
            }
            if self.canonicalize {
                std::borrow::Cow::<Path>::Owned(path.canonicalize().unwrap_or_else(|_| {
                    eprintln!("Error: The {path:?} directory does not exist");
                    std::process::exit(1)
                }))
            } else {
                std::borrow::Cow::<Path>::Borrowed(path)
            }
        });

        // Search in directories
        rayon::scope(move |s| {
            for dir in dirs {
                let sender = sender.clone();
                s.spawn(move |_| search_dir(dir, self, sender, 0));
            }
            drop(sender);
            receive_paths(receiver, self)
        })
    }
}

#[profi::profile]
fn search_dir(path: impl AsRef<Path>, search: &Search, sender: Sender, depth: usize) {
    let path = path.as_ref();

    let Ok(read) = std::fs::read_dir(path) else {
        if search.verbose {
            eprintln!("Could not read {path:?}");
        }
        return;
    };

    rayon::scope(|s| {
        for entry in read.flatten() {
            let Some((result, is_dir)) = is_result(entry, search) else {
                continue;
            };
            if let Some(result) = result {
                sender.send(result).unwrap();
            }
            if let Some(path) = is_dir {
                if depth > search.max_depth {
                    search_dir(path, search, sender.clone(), depth);
                    continue;
                }
                s.spawn(|_| search_dir(path, search, sender.clone(), depth + 1));
            }
        }
    });
}

#[profi::profile]
fn is_result(
    entry: std::fs::DirEntry,
    search: &Search,
) -> Option<(Option<SearchResult>, Option<Box<Path>>)> {
    // Get entry name
    let path = entry.path();

    if !search.explicit_ignore.is_empty() {
        let canonicalized = path.canonicalize().ok()?;
        let ignore = |entry: &std::path::PathBuf| {
            if entry.is_absolute() {
                entry == &canonicalized
            } else {
                entry.file_name() == path.file_name()
            }
        };
        if search.explicit_ignore.iter().any(ignore) {
            return None;
        }
    }

    let is_hidden = || {
        #[cfg(unix)]
        {
            is_hidden(&path)
        }
        #[cfg(windows)]
        {
            is_hidden(&entry)
        }
    };

    if !search.hidden && is_hidden() {
        return None;
    }

    // Read type of file and check if it should be added to search results
    let is_dir = matches!(entry.file_type(), Ok(ftype) if ftype.is_dir());
    let ftype = match search.ftype {
        FileType::All => true,
        FileType::Dir => is_dir,
        FileType::File => !is_dir,
    };

    let Some(fname) = file_name(&path) else {
        return Some((None, is_dir.then_some(path.into_boxed_path())));
    };
    let fname = fname.to_string_lossy();
    let sname: std::borrow::Cow<str> = if search.case_sensitive {
        fname.as_ref().into()
    } else {
        fname.to_ascii_lowercase().into()
    };

    let starts = || search.starts.is_empty() || sname.starts_with(&search.starts);
    let ends = || search.ends.is_empty() || sname.ends_with(&search.ends);

    if ftype && starts() && ends() {
        let (equals, contains) = {
            if search.finder.find(sname.as_bytes()).is_none() {
                (false, false)
            } else {
                (sname.len() == search.name.len(), true)
            }
        };
        // If file name is equal to search name, write it to the "Exact" buffer
        if equals {
            return Some((
                Some(SearchResult::exact(path.to_string_lossy().into_owned())),
                is_dir.then_some(path.into_boxed_path()),
            ));
        }
        // If file name contains the search name, write it to the "Contains" buffer
        else if !search.exact && contains {
            let s = if search.output == Output::Normal {
                crate::print::format_with_highlight(&fname, &sname, &path, search)
            } else {
                path.to_string_lossy().into_owned()
            };
            return Some((
                Some(SearchResult::contains(s)),
                is_dir.then_some(path.into_boxed_path()),
            ));
        }
    }
    Some((None, is_dir.then_some(path.into_boxed_path())))
}

#[profi::profile]
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

/// from https://github.com/BurntSushi/ripgrep/blob/master/crates/ignore/src/pathutil.rs
///
/// Returns true if and only if this entry is considered to be hidden.
///
/// This only returns true if the base name of the path starts with a `.`.
///
/// On Unix, this implements a more optimized check.
#[cfg(unix)]
#[inline(always)]
#[profi::profile]
pub(crate) fn is_hidden(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    file_name(path).is_some_and(|name| name.as_bytes().first().copied() == Some(b'.'))
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
#[inline(always)]
#[profi::profile]
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
#[profi::profile]
#[cfg(unix)]
#[inline(always)]
pub(crate) fn file_name(path: &Path) -> Option<&std::ffi::OsStr> {
    use std::os::unix::ffi::OsStrExt;

    let path = path.as_os_str().as_bytes();
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
#[inline(always)]
#[profi::profile]
pub(crate) fn file_name<'a, P: AsRef<Path> + ?Sized>(path: &'a P) -> Option<&'a std::ffi::OsStr> {
    path.as_ref().file_name()
}
