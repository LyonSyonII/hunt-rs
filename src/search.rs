use rayon::iter::{ParallelBridge, ParallelIterator};
use std::path::{Path, PathBuf};

use crate::structs::{Buffer, FileType, Output, Search};

static FOUND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Search {
    pub fn search(&self) {
        // If no limit, search current directory
        if !self.limit {
            search_path(&self.current_dir, self);
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
            dirs.par_bridge().for_each(|dir| {
                search_path(dir.as_ref(), self)
            });
        };
    }
}

fn search_path(dir: &Path, search: &Search) {
    if let Ok(read) = std::fs::read_dir(dir) {
        read.flatten().par_bridge().for_each(|entry| {
            search_dir(entry, search);
        })
    } else if search.verbose {
        eprintln!("Could not read {:?}", dir);
    }
}

fn search_dir(entry: std::fs::DirEntry, search: &Search) {
    // Get entry name
    let fname = if search.case_sensitive {
        entry.file_name()
    } else {
        entry.file_name().to_ascii_lowercase()
    };
    let fname = fname.to_string_lossy();
    let path = entry.path();
    
    if search.explicit_ignore.binary_search(&path).is_ok() {
        return;
    }

    if !search.hidden && (is_hidden(&entry) || search.hardcoded_ignore.binary_search_by(|p| std::path::Path::new(p).cmp(&path)).is_ok()) {
        return;
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
    if starts && ends && ftype {
        // If file name is equal to search name, write it to the "Exact" buffer
        if fname == search.name {
            print_var(
                &mut search.buffers.0.lock(),
                search.first,
                path.clone(),
                search.output,
            );
        }
        // If file name contains the search name, write it to the "Contains" buffer
        else if !search.exact && fname.contains(&search.name) {
            print_var(
                &mut search.buffers.1.lock(),
                search.first,
                path.clone(),
                search.output,
            );
        }
    }

    // If entry is not a directory, stop function
    if !is_dir {
        return;
    }

    if let Ok(read) = std::fs::read_dir(&path) {
        read.flatten().par_bridge().for_each(|entry| {
            search_dir(entry, search);
        })
    } else if search.verbose {
        eprintln!("Could not read {:?}", path);
    }
}

fn print_var(var: &mut Buffer, first: bool, path: PathBuf, output: Output) {
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