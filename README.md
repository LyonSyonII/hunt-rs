# Hunt
Hunt is a (highly-opinionated) simplified Find command made with Rust.  
It searches a file/folder by name on the entire drive.

## Usage
    hunt [OPTIONS] <NAME>

### Options
    -e, --exact    Only search for exactly matching occurrences
    -f, --first    Stop when first occurrence is found
    -h, --help     Print help information

### Args
    <NAME>  Name of the file/folder to search
    
    <LIMIT_TO_DIRS>...
            Directories where you want to search
            If provided, hunt will only search there
            
            These directories are treated independently, so if one is nested into another the
            search will be done two times:  
            
            e.g. "hunt somefile /home/user /home/user/downloads" will search in the home directory, and because /home/user/downloads is inside it, /downloads will be traversed two times


## Why I made it?
I found I used the `find` command just to search one file, so I wanted a simpler and faster option.

Hunt is multithreaded, so it's a lot faster than `find`, and more reliable than `locate` (recent files cannot be found with it).

## Installation
First check that you have (Rust)[https://www.rust-lang.org/] installed, then run

```
cargo install hunt
```

## Benchmarks
This benchmarks are done in a system with approximately 2,762,223 files, with a Network Drive and an external one.  
Results on other systems may vary, so take this comparisons as a guide.  
(All benchmarks have been done multiple times and the average has been taken)

### Searching file in ~/
#### Hunt
Find only first occurrence of a heavily nested file from the home directory.

```
~ ❯ time hunt -f -e SomeFile ~/
/home/lyon/.wine/drive_c/Program Files (x86)/Internet Explorer/SomeFile


________________________________________________________
Executed in   33,38 millis    fish           external
   usr time  107,91 millis    1,17 millis  106,74 millis
   sys time   36,07 millis    0,00 millis   36,07 millis
```

#### Find
```
~ ❯ time find ~/ -name SomeFile -print -quit
./.wine/drive_c/Program Files (x86)/Internet Explorer/SomeFile

________________________________________________________
Executed in    1,09 secs      fish           external
   usr time  245,30 millis    1,24 millis  244,06 millis
   sys time  378,23 millis    0,00 millis  378,23 millis
```

#### Locate
```
~ ❯ time locate -n 1 -A SomeFile
/home/lyon/.wine/drive_c/Program Files (x86)/Internet Explorer/SomeFile

________________________________________________________
Executed in  253,09 millis    fish           external
   usr time  322,54 millis    1,23 millis  321,31 millis
   sys time   10,23 millis    0,00 millis   10,23 millis

```

#### Fd
```
~ ❯ time fd -H --max-results 1 -c never SomeFile .
./.wine/drive_c/Program Files (x86)/Internet Explorer/SomeFile

________________________________________________________
Executed in  177,23 millis    fish           external
   usr time  961,45 millis    1,20 millis  960,25 millis
   sys time  931,60 millis    0,00 millis  931,60 millis
```

### Searching all files that contain SomeFile
Find all occurrences of "SomeFile" from the root directory.

#### Hunt
```
/ ❯ time hunt SomeFile
Contains:
/home/lyon/Downloads/abcdefgSomeFileeee
/SomeFileIsHere
/mnt/Files/--SomeFile--

Exact:
/home/lyon/.wine/drive_c/Program Files (x86)/Internet Explorer/SomeFile


________________________________________________________
Executed in  560,58 millis    fish           external
   usr time    1,95 secs    501,00 micros    1,95 secs
   sys time    2,67 secs    276,00 micros    2,67 secs
```

#### Find
```
/ ❯ time sudo find -name "*SomeFile*"
./mnt/Files/--SomeFile--
./home/lyon/Downloads/abcdefgSomeFileeee
./home/lyon/.wine/drive_c/Program Files (x86)/Internet Explorer/SomeFile
./SomeFileIsHere

________________________________________________________
Executed in    2,48 secs    fish           external
   usr time    1,22 secs    0,00 millis    1,22 secs
   sys time    1,31 secs    1,50 millis    1,31 secs
```

#### Locate
```
/ ❯ time locate SomeFile
/SomeFileIsHere
/home/lyon/.wine/drive_c/Program Files (x86)/Internet Explorer/SomeFile
/home/lyon/Downloads/abcdefgSomeFileeee

________________________________________________________
Executed in  488,23 millis    fish           external
   usr time  550,95 millis  432,00 micros  550,52 millis
   sys time   13,70 millis  238,00 micros   13,47 millis
```
Locate is obviously faster, as it doesn't traverse all the files (it is supported by a db), but as you can see files on other drives are not detected, meaning "/mnt/Files/--SomeFile--" is not in the list. 

#### Fd
```
/ ❯ time fd -H -c never SomeFile
SomeFileIsHere
home/lyon/.wine/drive_c/Program Files (x86)/Internet Explorer/SomeFile
home/lyon/Downloads/abcdefgSomeFileeee
mnt/Files/--SomeFile--

________________________________________________________
Executed in    1,59 secs    fish           external
   usr time    5,28 secs  478,00 micros    5,28 secs
   sys time    9,01 secs  264,00 micros    9,01 secs
```

### Conclusion
Hunt is faster than other alternatives if you don't need a lot of features (like regex).  
Think of it as a simple "where did I put that file?" solution.