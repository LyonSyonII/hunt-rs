use crate::structs::{Output, Search};
use colored::Colorize;
use rayon::prelude::ParallelSliceMut;
use std::io::Write;

impl Search {
    pub fn print_results(self) -> std::io::Result<()> {
        print_results(self)
    }
}

fn print_results(search: Search) -> std::io::Result<()> {
    let (mut ex, mut co) = (search.buffers.0.lock(), search.buffers.1.lock());

    if ex.is_empty() && co.is_empty() && search.output == Output::Normal {
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
    for path in co.iter() {
        print_with_highlight(&mut stdout, path, &search)?;
    }
    if search.output == Output::Normal {
        writeln!(stdout, "\nExact:")?;
    }
    for path in ex.iter() {
        writeln!(stdout, "{}", path.display())?;
    }
    Ok(())
}

fn print_with_highlight(
    stdout: &mut std::io::BufWriter<std::io::StdoutLock>,
    path: &std::path::Path,
    search: &Search,
) -> std::io::Result<()> {
    if search.output != Output::Normal {
        return writeln!(stdout, "{}", path.display());
    }

    let ancestors = path.parent().unwrap();
    let fname = path.file_name().unwrap().to_string_lossy();
    let sname: std::borrow::Cow<'_, str> = if search.case_sensitive {
        fname.as_ref().into()
    } else {
        fname.to_ascii_lowercase().into()
    };

    let get_start_end = |s: &str| {
        let start = sname.find(s).unwrap();
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

    let ancestors = ancestors.display();
    let sep = std::path::MAIN_SEPARATOR;
    let starts = &fname[starts_idx.0..starts_idx.1].bright_purple().bold();
    let starts_to_name = &fname[starts_idx.1..name_idx.0];
    let name = &fname[name_idx.0..name_idx.1].bright_red().bold();
    let name_to_ends = &fname[name_idx.1..ends_idx.0];
    let ends = &fname[ends_idx.0..ends_idx.1].bright_purple().bold();
    let empty_ends = &fname[ends_idx.1..]; // Needed because we don't want to highlight the end of the path if "--ends" is not specified
    writeln!(
        stdout,
        "{ancestors}{sep}{starts}{starts_to_name}{name}{name_to_ends}{ends}{empty_ends}"
    )
}
