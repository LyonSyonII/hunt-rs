# Hunt
Hunt is a simplified Find command made with Rust.  
It searches a file/folder by name on the entire drive.

## Usage
    hunt [OPTIONS] <NAME>

### Options
    -e, --exact    Only search for exactly matching occurrences
    -f, --first    Stop when first occurrence is found
    -h, --help     Print help information

### Args
    <NAME>    Name of the file/folder to search

## Why I made it?
I found I used the `find` command just to search one file, so I wanted a simpler and faster option.

Hunt is multithreaded, so it's a lot faster than `find`, and more reliable than `locate` (recent files cannot be found with it).

## Installation
First check that you have (Rust)[https://www.rust-lang.org/] installed, then run  
```cargo install hunt```
