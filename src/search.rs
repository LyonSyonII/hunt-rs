use rayon::iter::{ParallelBridge, ParallelIterator};
use std::{path::{Path, PathBuf}, sync::atomic::{AtomicBool, AtomicUsize}};

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
                search_path(dir.as_ref(), self);
                // search_path_queue(dir.as_ref(), self)
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

    if search.explicit_ignore.contains(&path) {
        return;
    }

    if !search.hidden && (is_hidden(&entry) || search.hardcoded_ignore.contains(path.as_path())) {
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
    // recursion_level.fetch_sub(1, Ordering::AcqRel);
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

/*
TODO: Improve stack algorithm, currently it's much slower
fn search_path_queue(path: &Path, search: &Search) {
    static STACK: parking_lot::Mutex<Vec<std::fs::DirEntry>> = parking_lot::Mutex::new(Vec::new());
    static ADDING: AtomicUsize = AtomicUsize::new(0);

    STACK.lock().extend(match path.read_dir() {
        Ok(ok) => ok.flatten(),
        Err(err) => return eprintln!("Could not read {:?}: {err}", path),
    });

    ADDING.store(STACK.lock().len(), std::sync::atomic::Ordering::Release);
    
    rayon::scope(|s| {
        while ADDING.load(std::sync::atomic::Ordering::Acquire) > 0 {
            if let Some(entry) = STACK.lock().pop() {
                s.spawn(move |_| {
                    // Check if file is what we're searching.
                    if check_file(&entry, search).is_err() { return }
                    
                    let path = entry.path();
                    if path.is_dir() {
                        let mut lock = STACK.lock();
                        let len = lock.len();
                        lock.extend(match path.read_dir() {
                            Ok(ok) => ok.flatten(),
                            Err(err) => {
                                if search.verbose {
                                    eprintln!("Could not read {:?}: {err}", path)
                                }
                                return
                            },
                        });
                        ADDING.fetch_add(lock.len() - len, std::sync::atomic::Ordering::AcqRel);
                    }
                    ADDING.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                })
            }
        }
    });
}

/// Checks file against search query.
fn check_file(entry: &std::fs::DirEntry, search: &Search) -> Result<bool, ()> {
    // Get entry name
    let fname = if search.case_sensitive {
        entry.file_name()
    } else {
        entry.file_name().to_ascii_lowercase()
    };
    let fname = fname.to_string_lossy();
    let path = entry.path();

    if search.explicit_ignore.contains(&path) {
        return Err(());
    }

    if !search.hidden && (is_hidden(entry) || search.hardcoded_ignore.contains(path.as_path())) {
        return Err(());
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
                path,
                search.output,
            );
        }
        // If file name contains the search name, write it to the "Contains" buffer
        else if !search.exact && fname.contains(&search.name) {
            print_var(
                &mut search.buffers.1.lock(),
                search.first,
                path,
                search.output,
            );
        }

        return Ok(true)
    }

    Ok(false)
}
*/
