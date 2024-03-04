use rayon::iter::{ParallelBridge, ParallelIterator};
use std::path::Path;
use crate::structs::{Buffer, Buffers, FileType, Output, Search};

static FOUND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Search {
    pub fn search(&self) -> Buffers {
        // If no limit, search current directory
        if !self.limit {
            let path = if self.canonicalize {
                std::env::current_dir().expect("Could not read current directory")
            } else {
                std::path::Path::new(".").to_owned()
            };
            return search_path(&path, self);
        }
        // Check if paths are valid and canonicalize if necessary
        let dirs = self.dirs.iter().map(|path| {
            if !path.exists() {
                eprintln!("ERROR: The {:?} directory does not exist", path);
                std::process::exit(1)
            }

            if self.canonicalize {
                std::borrow::Cow::Owned(path.canonicalize().unwrap_or_else(|_| {
                    eprintln!("ERROR: The {:?} directory does not exist", path);
                    std::process::exit(1)
                }))
            } else {
                std::borrow::Cow::Borrowed(path)
            }
        });
        // Search in directories
        par_fold(dirs, |dir| search_path(dir.as_ref(), self))
    }
}

fn search_path(dir: &Path, search: &Search) -> Buffers {
    if let Ok(read) = std::fs::read_dir(dir) {
        return par_fold(read.flatten(), |entry| search_dir(entry, search));
    } else if search.verbose {
        eprintln!("Could not read {:?}", dir);
    }
    new_buffers()
}

fn search_dir(entry: std::fs::DirEntry, search: &Search) -> Buffers {
    // Get entry name
    let fname = if search.case_sensitive {
        entry.file_name()
    } else {
        entry.file_name().to_ascii_lowercase()
    };
    let fname = fname.to_string_lossy();
    let path = entry.path();

    if search.explicit_ignore.binary_search(&path).is_ok() {
        return new_buffers();
    }

    let hardcoded = || {
        search
            .hardcoded_ignore
            .binary_search_by(|p| std::path::Path::new(p).cmp(&path))
            .is_ok()
    };

    if !search.hidden && (is_hidden(&entry) || hardcoded()) {
        return new_buffers();
    }

    // Read type of file and check if it should be added to search results
    let is_dir = matches!(entry.file_type(), Ok(ftype) if ftype.is_dir());
    let ftype = match search.ftype {
        FileType::Dir => is_dir,
        FileType::File => !is_dir,
        FileType::All => true,
    };

    let starts = search.starts.is_empty() || fname.starts_with(&search.starts);
    let ends = search.ends.is_empty() || fname.ends_with(&search.ends);
    let mut buffers = new_buffers();
    
    if starts && ends && ftype {
        // If file name is equal to search name, write it to the "Exact" buffer
        if fname == search.name {
            print_var(&mut buffers.0, search.first, path.clone(), search.output);
        }
        // If file name contains the search name, write it to the "Contains" buffer
        else if !search.exact && fname.contains(&search.name) {
            print_var(&mut buffers.1, search.first, path.clone(), search.output);
        }
    }
    
    // If entry is not a directory, stop function
    if !is_dir {
        return buffers;
    }

    if let Ok(read) = std::fs::read_dir(&path) {
        let b = par_fold(read.flatten(), |entry| search_dir(entry, search));
        let (mut buffers, b) = if buffers.0.len() + buffers.1.len() > b.0.len() + b.1.len() {
            (buffers, b)
        } else {
            (b, buffers)
        };
        buffers.0.extend(b.0);
        buffers.1.extend(b.1);
        return buffers;
    } else if search.verbose {
        eprintln!("Could not read {:?}", path);
    }

    buffers
}

fn new_buffers() -> Buffers {
    (Buffer::new(), Buffer::new())
}

fn par_fold<I, F, T>(iter: I, map: F) -> Buffers
where
    I: IntoIterator<Item = T>,
    <I as IntoIterator>::IntoIter: Send,
    F: Fn(T) -> Buffers + Sync + Send,
    T: Send,
{
    use rayon::prelude::*;
    iter.into_iter()
        .par_bridge()
        .map(map)
        .reduce_with(|mut acc, results| {
            acc.0.extend(results.0);
            acc.1.extend(results.1);
            acc
        }).unwrap_or_default()
}

fn print_var(var: &mut Buffer, first: bool, path: std::path::PathBuf, output: Output) {
    if first {
        let found = FOUND.load(std::sync::atomic::Ordering::Acquire);
        if found {
            return;
        }

        FOUND.store(true, std::sync::atomic::Ordering::Release);
        println!("{}", path.display());
        std::process::exit(0)
    } else if output == Output::SuperSimple {
        println!("{}", path.display());
    } else {
        var.push(path);
    }
}

// OS-variable functions
#[cfg(windows)]
fn is_hidden(entry: &std::fs::DirEntry) -> bool {
    use std::os::windows::prelude::*;
    let metadata = entry.metadata().unwrap();
    let attributes = metadata.file_attributes();

    attributes & 0x2 > 0
}

#[cfg(unix)]
fn is_hidden(entry: &std::fs::DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with('.')
}
