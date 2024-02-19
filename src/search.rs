use rayon::iter::{ParallelBridge, ParallelIterator};
use std::path::{Path, PathBuf};

use crate::structs::{Buffer, Buffers, FileType, Output, Search};

static FOUND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Search {
    pub fn search(&self) -> Buffers {
        // If no limit, search current directory
        if !self.limit {
            search_path(&self.current_dir, self)
        } else {
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
            dirs.par_bridge()
                .fold(
                    || (Vec::new(), Vec::new()),
                    |mut v, dir| {
                        let results = search_path(dir.as_ref(), self);
                        v.0.extend(results.0);
                        v.1.extend(results.1);
                        v
                    },
                )
                .reduce(
                    || (Vec::new(), Vec::new()),
                    |mut acc, v| {
                        acc.0.extend(v.0);
                        acc.1.extend(v.1);
                        acc
                    },
                )
        }
    }
}

fn search_path(dir: &Path, search: &Search) -> Buffers {
    if let Ok(read) = std::fs::read_dir(dir) {
        return read
            .flatten()
            .par_bridge()
            .fold(
                || (Vec::new(), Vec::new()),
                |mut v, entry| {
                    let results = search_dir(entry, search);
                    v.0.extend(results.0);
                    v.1.extend(results.1);
                    v
                },
            )
            .reduce(
                || (Vec::new(), Vec::new()),
                |mut acc, v| {
                    acc.0.extend(v.0);
                    acc.1.extend(v.1);
                    acc
                },
            );
    } else if search.verbose {
        eprintln!("Could not read {:?}", dir);
    }
    (Vec::new(), Vec::new())
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
        return (Vec::new(), Vec::new());
    }

    let hardcoded = || {
        search
            .hardcoded_ignore
            .binary_search_by(|p| std::path::Path::new(p).cmp(&path))
            .is_ok()
    };

    if !search.hidden && (is_hidden(&entry) || hardcoded()) {
        return (Vec::new(), Vec::new());
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
    let mut buffers = (Vec::new(), Vec::new());

    if starts && ends && ftype {
        // If file name is equal to search name, write it to the "Exact" buffer
        if fname == search.name {
            print_var(&mut buffers.0, search.first, &path, search.output);
        }
        // If file name contains the search name, write it to the "Contains" buffer
        else if !search.exact && fname.contains(&search.name) {
            print_var(&mut buffers.1, search.first, &path, search.output);
        }
    }

    // If entry is not a directory, stop function
    if !is_dir {
        return buffers;
    }

    if let Ok(read) = std::fs::read_dir(&path) {
        let b = read
            .flatten()
            .par_bridge()
            .fold(
                || (Buffer::new(), Buffer::new()),
                |mut v, entry| {
                    let results = search_dir(entry, search);
                    v.0.extend(results.0);
                    v.1.extend(results.1);
                    v
                },
            )
            .reduce(
                || (Vec::new(), Vec::new()),
                |mut acc, v| {
                    acc.0.extend(v.0);
                    acc.1.extend(v.1);
                    acc
                },
            );
        buffers.0.extend(b.0);
        buffers.1.extend(b.1);
    } else if search.verbose {
        eprintln!("Could not read {:?}", path);
    }
    buffers
}

fn print_var(var: &mut Buffer, first: bool, path: &Path, output: Output) {
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
        var.push(path.to_owned());
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
