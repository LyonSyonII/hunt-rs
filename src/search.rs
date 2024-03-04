use rayon::iter::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use std::path::{Path, PathBuf};
use crate::structs::{Buffer, Buffers, FileType, Output, Search};

static FOUND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

type Receiver = std::sync::mpsc::Receiver<String>;
type Sender = std::sync::mpsc::Sender<String>;

impl Search {
    pub fn search(&self) -> Buffers {
        let (sender, receiver) = std::sync::mpsc::channel();

        // If no limit, search current directory
        if !self.limit {
            let path = if self.canonicalize {
                std::env::current_dir().expect("Could not read current directory")
            } else {
                std::path::Path::new(".").to_owned()
            };
            search_path(&path, self, sender.clone());
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
        // par_fold(dirs, |dir| search_path(dir.as_ref(), self, sender.clone()));
        let received = rayon::scope(move |s| {
            s.spawn(move |_| dirs.into_iter().par_bridge().for_each(|dir| search_path(dir.as_ref(), self, sender.clone())));
            receive_paths(receiver, self)
        });
        
        (Vec::new(), received)
    }
}

fn receive_paths(receiver: Receiver, search: &Search) -> Buffer {
    let stdout = std::io::stdout();
    let mut stdout = std::io::BufWriter::new(stdout.lock());
    let mut received = Vec::new();
    
    while let Ok(path) = receiver.recv() {
        use std::io::Write;
        if search.first {
            println!("{path}");
            std::process::exit(0)
        }
         else if search.output == Output::SuperSimple {
            writeln!(stdout, "{path}").unwrap();
        } else {
            received.push(path);
        }
    }
    received
}

fn search_path(dir: &Path, search: &Search, sender: Sender) {
    if let Ok(read) = std::fs::read_dir(dir) {
        read.flatten().par_bridge().for_each(|entry| search_dir(entry, search, sender.clone()));
        // return par_fold(read.flatten(), |entry| search_dir(entry, search, sender.clone()));
    } else if search.verbose {
        eprintln!("Could not read {:?}", dir);
    }
}

fn search_dir(entry: std::fs::DirEntry, search: &Search, sender: Sender) {
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
            // TODO: Exact
            let s = crate::print::format_with_highlight(&fname, &sname, &path, search);
            sender.send(s).unwrap();
            // print_var(&sender, search.first, path.clone(), search.output);
        }
        // If file name contains the search name, write it to the "Contains" buffer
        else if !search.exact && sname.contains(&search.name) {
            // TODO: Contains
            let s = crate::print::format_with_highlight(&fname, &sname, &path, search);
            sender.send(s).unwrap();
        }
    }
    
    // If entry is not a directory, stop function
    if !is_dir {
        return;
    }

    if let Ok(read) = std::fs::read_dir(&path) {
        read.flatten().par_bridge().for_each(|entry| search_dir(entry, search, sender.clone()));
    } else if search.verbose {
        eprintln!("Could not read {:?}", path);
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
