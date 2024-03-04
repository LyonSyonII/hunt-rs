use crate::structs::{Buffers, Output, Search};
// use yansi::{Paint, Style};
use rayon::prelude::ParallelSliceMut;
use std::io::Write;

impl Search {
    pub fn print_results(self, buffers: Buffers) -> std::io::Result<()> {
        if self.output == Output::SuperSimple {
            return Ok(());
        }
        print_results(self, buffers)
    }
}

fn print_results(search: Search, buffers: Buffers) -> std::io::Result<()> {
    let (mut ex, mut co) = buffers;

    if ex.is_empty() && co.is_empty() && search.output == Output::Normal {
        println!("File not found");
        return Ok(());
    }
    
    crate::perf! {
        ctx = "sort";
        rayon::join(|| co.par_sort(), || ex.par_sort());
    }

    // Print results
    let stdout = std::io::stdout().lock();
    let mut stdout = std::io::BufWriter::new(stdout);
    
    crate::perf! {
        ctx = "print";

        if search.output == Output::Normal {
            writeln!(stdout, "Contains:")?;
        }
        for path in co.iter() {
            print_with_highlight(&mut stdout, path, &search)?;
        }
        if search.output == Output::Normal {
            writeln!(stdout, "\nExact:")?;
        }
        for path in ex.iter() {
            writeln!(stdout, "{}", path.display())?;
        }
    }
    Ok(())
}

fn print_with_highlight(
    stdout: &mut impl std::io::Write,
    path: &std::path::Path,
    search: &Search,
) -> std::io::Result<()> {
    if search.output != Output::Normal {
        return writeln!(stdout, "{}", path.display());
    }
    
    
    crate::perf! {
        disable;
        ctx = "names highlight";
        
        crate::perf! {
            ctx = "ancestors";
            let ancestors = path.parent().unwrap();
        }
        crate::perf! {
            ctx = "fname";
            let fname = path.file_name().unwrap();
        }
        crate::perf! {
            ctx = "to_ascii";
            let sname: std::borrow::Cow<std::ffi::OsStr> = if search.case_sensitive {
                // fname.as_ref().into()
                fname.into()
            } else {
                fname.to_ascii_lowercase().into()
            };
        }
    }
    
    crate::perf! {
        disable;
        ctx = "get highlight";

        let get_start_end = |s: &str| {
            use clap_lex::OsStrExt;

            let start = sname.find(s).unwrap();
            (start, start + s.len())
        };
        
        let starts_idx = if search.starts.is_empty() { 
            (0, 0)
        } else {
            get_start_end(&search.starts)
        };
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
    }
    
    crate::perf! {
        disable;
        ctx = "build highlight";

        use colored::Colorize;

        let bytes = fname.as_encoded_bytes();
        let get_str = |range| {
            unsafe {
                std::ffi::OsStr::from_encoded_bytes_unchecked(bytes[range])
            }
        };

        let ancestors = ancestors.display();
        let sep = std::path::MAIN_SEPARATOR;
        let starts = get_str(starts_idx.0..starts_idx.1).bright_magenta().bold();
        let starts = &std::ffi::OsStr::from_encoded_bytes_unchecked(bytes[starts_idx.0..starts_idx.1]).bright_magenta().bold();
        let starts_to_name = &fname[starts_idx.1..name_idx.0];
        let name = &fname[name_idx.0..name_idx.1].bright_red().bold();
        let name_to_ends = &fname[name_idx.1..ends_idx.0];
        let ends = &fname[ends_idx.0..ends_idx.1].bright_magenta().bold();
        let empty_ends = &fname[ends_idx.1..]; // Needed because we don't want to highlight the end of the path if "--ends" is not specified
    }
    crate::perf! {
        disable;
        ctx = "print highlight";

        writeln!(
            stdout,
            "{ancestors}{sep}{starts}{starts_to_name}{name}{name_to_ends}{ends}{empty_ends}"
        )?;
    }
    Ok(())
}
