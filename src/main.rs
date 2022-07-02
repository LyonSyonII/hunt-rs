mod structs;
use structs::*;

use parking_lot::Mutex;
use colored::Colorize;
use rayon::{
    iter::{ParallelBridge, ParallelIterator},
    slice::ParallelSliceMut,
};
use std::{
    io::{Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

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

fn search_dir(entry: std::fs::DirEntry, search: &Search, buffers: &Buffers) {
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
        FileType::Dir => {
            is_dir
        }
        FileType::File => {
            !is_dir
        }
        FileType::All => true,
    };
    
    let starts = search.starts.is_empty() || fname.starts_with(&search.starts);
    let ends = search.ends.is_empty() || fname.ends_with(&search.ends);
    if starts && ends && ftype {
        // If file name is equal to search name, write it to the "Exact" buffer
        if fname == search.name {
            print_var(&mut buffers.0.lock(), search.first, path.clone());
        } 
        // If file name contains the search name, write it to the "Contains" buffer
        else if !search.exact && fname.contains(&search.name) {
            print_var(&mut buffers.1.lock(), search.first, path.clone());
        }
    }
    
    // If entry is not a directory, stop function 
    if !is_dir {
        return;
    }
    // If entry is a directory, search inside it
    if let Ok(read) = std::fs::read_dir(&path) {
        read.flatten().par_bridge().for_each(|entry| {
            search_dir(entry, search, buffers);
        })
    } else if search.verbose {
        eprintln!("Could not read {:?}", path);
    }
}

fn search_path(dir: &Path, search: &Search, buffers: &Buffers) {
    if let Ok(read) = std::fs::read_dir(dir) {
        read.flatten().par_bridge().for_each(|entry| {
            search_dir(entry, search, buffers);
        })
    } else if search.verbose {
        eprintln!("Could not read {:?}", dir);
    }
}

fn print_with_highlight(stdout: &mut std::io::BufWriter<std::io::StdoutLock>, path: &Path, search: &Search, simple: Output, case_sensitive: bool) -> std::io::Result<()> {
    if simple == Output::Normal {
        let ancestors = path.parent().unwrap();
        let path = path.file_name().unwrap().to_string_lossy();
        let result = if case_sensitive {
            path.to_string()
        } else {
            path.to_ascii_lowercase()
        };
        
        let get_start_end = |s: &str| {
            let start = result.find(s).unwrap();
            (start, start + s.len())
        };
        
        let starts_idx = get_start_end(&search.starts);
        let name_idx = if search.name.is_empty() {
            (starts_idx.1, starts_idx.1)
        } else {
            get_start_end(&search.name)
        };
        let ends_idx = if search.ends.is_empty() {
            (name_idx.1, name_idx.1)
        } else {
            get_start_end(&search.ends)
        };

        //println!("Starts: {starts:?}, Name: {name:?}, Ends: {ends:?}");
        let ancestors = ancestors.display();
        let sep = std::path::MAIN_SEPARATOR;
        let starts = &path[starts_idx.0..starts_idx.1].bright_purple().bold();
        let starts_to_name = &path[starts_idx.1..name_idx.0];
        let name = &path[name_idx.0..name_idx.1].bright_red().bold();
        let name_to_ends = &path[name_idx.1..ends_idx.0];
        let ends = &path[ends_idx.0..ends_idx.1].bright_purple().bold();
        let empty_ends = &path[ends_idx.1..]; // Needed because we don't want to highlight the end of the path if "--ends" is not specified
        return writeln!(stdout, "{ancestors}{sep}{starts}{starts_to_name}{name}{name_to_ends}{ends}{empty_ends}");
    } 
    
    writeln!(stdout, "{}", path.display())
}

fn main() -> std::io::Result<()> {
    let search = Cli::run();
    let buffers: Buffers = (Mutex::new(Vec::new()), Mutex::new(Vec::new()));
    
    // If no limit, search current directory
    if !search.limit {
        search_path(&search.current_dir, &search, &buffers);
    } else {
        // Check if paths are valid
        let dirs = search.dirs.iter().map(|s| {
            std::fs::canonicalize(s).unwrap_or_else(|_| {
                eprintln!("ERROR: The {:?} directory does not exist", s);
                std::process::exit(1);
            })
        });
        // Search in directories
        dirs.par_bridge().for_each(|dir| {
            search_path(&dir, &search, &buffers);
        });
    };

    // Get results and sort them
    let (mut ex, mut co) = (buffers.0.into_inner(), buffers.1.into_inner());

    if ex.is_empty() && co.is_empty() {
        println!("File not found");
        return Ok(());
    }

    if search.output != Output::SuperSimple {
        co.par_sort_unstable();
        ex.par_sort_unstable();
    }
    
    // Print results
    let stdout = std::io::stdout().lock();
    let mut stdout = std::io::BufWriter::new(stdout);

    if search.output == Output::Normal {
        writeln!(stdout, "Contains:")?;
    }
    for path in co {
        print_with_highlight(&mut stdout, &path, &search, search.output, search.case_sensitive)?;
    }
    if search.output == Output::Normal { 
        writeln!(stdout, "\nExact:")?; 
    }
    for path in ex {
        writeln!(stdout, "{}", path.display())?;
    }
    Ok(())
}