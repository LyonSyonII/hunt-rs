# Hunt
Hunt is a (highly-opinionated) simplified Find command made with Rust.  
It searches a file/folder by name on the entire drive, collecting the exact matches and the ones that contain the query.  
Search results will be sorted alphabetically.

For example, `hunt SomeFile` will search "SomeFile" from the root directory, and an output could be:
    
    Contains:
    /SomeFileIsHere
    /home/lyon/Downloads/abcdefgSomeFileeee
    /mnt/Files/--SomeFile--

    Exact:
    /home/lyon/SomeFile

If the --first flag is set, the order in which the file will be searched is [current_dir, home_dir, root].  
If you're already in one of these directories, "current_dir" will be skipped.

If the --hidden flag is **not** set, hidden files/directories will be skipped, as well as this ones: ["/proc", "/root", "/boot", "/dev", "/lib", "/lib64", "/lost+found", "/run", "/sbin", "/sys", "/tmp", "/var/tmp", "/var/lib", "/var/log", "/var/db", "/var/cache", "/etc/pacman.d", "/etc/sudoers.d" and "/etc/audit"]

## Usage
    hunt [OPTIONS] <NAME> <LIMIT_TO_DIRS>...

### Options
    -e, --exact    Only search for exactly matching occurrences, any file only 
                   containing the query will be skipped
            
                    e.g. if query is "SomeFile", "I'mSomeFile" will be skipped, 
                    as its name contains more letters than the search.

    -f, --first    Stop when first occurrence is found

    -h, --hidden   If enabled, it searches inside hidden and ignored directories.

                   The list of ignored directories is:
                   "/proc", "/root", "/boot", "/dev", "/lib", "/lib64", 
                   "/lost+found", "/run", "/sbin", "/sys", "/tmp", "/var/tmp",
                   "/var/lib", "/var/log", "/var/db", "/var/cache", 
                   "/etc/pacman.d", "/etc/sudoers.d" and "/etc/audit"

    -i, --ignore <IGNORE_DIRS>
                   Search ignores this directories. The format is:
                   -i dir1,dir2,dir3,... (without spaces)

    -S, --starts <STARTS_WITH> 
                   Only files that start with this will be found
        
    -E, --ends   <ENDS_WITH>
                   Only files that end with this will be found

    -t, --type   <FILE_TYPE>
                   Specifies the type of the file
                   'f' -> file
                   'd' -> directory

    -v, --verbose  Print verbose output
                   It'll show all errors found: e.g. "Could not read /proc/81261/map_files"
    
    -s, --simple   Prints without formatting (without "Contains:" and "Exact:")
                   Useful for pairing it with other commands like xargs

        --help     Print help information

### Args
    <NAME>  Name of the file/folder to search
    
    <LIMIT_TO_DIRS>...
            Directories where you want to search
            If provided, hunt will only search there
            
            These directories are treated independently, so if one is nested into another the
            search will be done two times:  
            
            e.g. "hunt somefile /home/user /home/user/downloads" will search in the home
            directory, and because /home/user/downloads is inside it, /downloads will be
            traversed two times

### Examples
* Search for a specific file on the whole system (hunt will stop once found)  
    
        hunt -f -e SomeFile

* Search for files containing "SomeFile"
    
        hunt SomeFile

* Search file in the home directory
    
        hunt -e SomeFile ~/

* Search file in the downloads and pictures directories
    
        hunt -e SomeFile ~/downloads ~/pictures

* Search all files that end with ".exe"
    
        hunt --ends .exe

* Search all files that end with ".exe" in the wine directory
    
        hunt --ends .exe ~/.wine

* Search all files that start with "." (all hidden files)
    
        hunt --starts .

* Search all files that end with ".exe", start with "M" and contain "wind" in the wine directory

        hunt --starts=M --ends=.exe wind ~/.wine

* Search a directory named "folder"
    
        hunt -t=d folder

* Search a file named "notfolder"
    
        hunt -t=f notfolder

* Remove all files named "SomeFile"
        
        hunt -s -e SomeFile | xargs rm -r

## Why I made it?
I found I used the `find` command just to search one file, so I wanted a simpler and faster option.

Hunt is multithreaded, so it's a lot faster than `find`, and more reliable than `locate` (recent files cannot be found with it).

## Installation
First check that you have [Rust](https://www.rust-lang.org/) installed, then run

    cargo install hunt

## Benchmarks
This benchmarks are done in a system with approximately 2,762,223 files, with a Network Drive and an external one.  
Results on other systems may vary, so take this comparisons as a guide.  
(All benchmarks have been done multiple times and the average has been taken, assume that the filesystem is in cache)

### Searching file in ~/
Find only first occurrence of a heavily nested file in a hidden folder from the home directory.

#### Hunt
-f -> --first, hunt will stop when first occurrence is found.  
-e -> --exact, hunt will only search for files/folders named "SomeFile", names that only contain the pattern will be skipped.  
-h -> --hidden, hunt will search all files, even hidden ones.
```
~ ❯ time hunt -f -e -h SomeFile ~/
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
Find all occurrences of "SomeFile" from the root directory (worst case scenario, checking all files in the system). 

#### Hunt
```
/ ❯ time hunt -h SomeFile
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