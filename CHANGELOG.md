## Version 2.0
### Breaking changes
- Updated to clap 4.3.0.
- Changed `case_sensitive` flag from `-c` to `-C`.
- Changed `hidden` flag from `-h` to `-H`.
- `help` can now be triggered by `-h` and `--help`.
- Added `--canonicalize` and `-c` flags
  - `hunt`'s output will now depend on the paths in `SEARCH_IN_DIRS`.  
  Only queries starting with `/` will be canonicallized.  
  - Addresses [#2](https://github.com/LyonSyonII/hunt-rs/issues/2).
- The `-ss` flag now prints the results incrementally, as no sorting is needed.  
  - The "File not found" message will only be shown when no `-s` flag is provided.
  - Addresses [#4](https://github.com/LyonSyonII/hunt-rs/issues/4).

### Other changes
- Fixed README examples to account for the flag changes.
- Updated multiple dependencies