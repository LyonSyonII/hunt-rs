use crate::structs::{Buffers, Output, Search};
use rayon::prelude::ParallelSliceMut;
use std::io::Write;

impl Search {
    pub fn print_results(self, buffers: Buffers) -> std::io::Result<()> {
        if self.output == Output::SuperSimple {
            return Ok(());
        }

        let stdout = std::io::stdout();
        let mut stdout = std::io::BufWriter::new(stdout.lock());

        let (mut ex, mut co) = buffers;
        if ex.is_empty() && co.is_empty() {
            if self.output == Output::Normal {
                writeln!(stdout, "File not found")?;
            }
            return Ok(());
        }
        crate::perf! {
            ctx = "sort";
            rayon::join(|| co.par_sort(), || ex.par_sort());
        }
        crate::perf! {
            ctx = "print";
            if self.output == Output::Normal {
                writeln!(stdout, "Contains:")?;
            }
            for path in co.into_iter() {
                writeln!(stdout, "{path}")?;
            }
            if self.output == Output::Normal {
                writeln!(stdout, "\nExact:")?;
            }
            for path in ex.into_iter() {
                writeln!(stdout, "{}", path.display())?;
            }
        }
        Ok(())
    }
}

pub fn print_with_highlight(
    stdout: &mut impl std::io::Write,
    fname: &str,
    sname: &str,
    path: &std::path::Path,
    search: &Search,
) -> std::io::Result<()> {
    let ancestors = path.parent().unwrap();

    let get_start_end = |s: &str| {
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
    
    use colored::Colorize;

    // let ancestors = ancestors.display();
    let sep = std::path::MAIN_SEPARATOR;
    let starts = &fname[starts_idx.0..starts_idx.1].bright_magenta().bold();
    let starts_to_name = &fname[starts_idx.1..name_idx.0];
    let name = &fname[name_idx.0..name_idx.1].bright_red().bold();
    let name_to_ends = &fname[name_idx.1..ends_idx.0];
    let ends = &fname[ends_idx.0..ends_idx.1].bright_magenta().bold();
    let empty_ends = &fname[ends_idx.1..]; // Needed because we don't want to highlight the end of the path if "--ends" is not specified
    
    if ancestors.as_os_str().len() > 1 || !ancestors.starts_with("/") {
        write!(stdout, "{}", ancestors.display())?;
    }
    write!(
        stdout,
        "{sep}{starts}{starts_to_name}{name}{name_to_ends}{ends}{empty_ends}"
    )
}

pub fn format_with_highlight(
    fname: &str,
    sname: &str,
    path: &std::path::Path,
    search: &Search,
) -> String {
    let mut buffer = Vec::new();
    print_with_highlight(&mut buffer, fname, sname, path, search).unwrap();
    unsafe { String::from_utf8_unchecked(buffer) }
}
