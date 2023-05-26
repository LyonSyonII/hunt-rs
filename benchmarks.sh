# Benchmark 1: Searching file in ~/
# File is in "/home/user/.wine/drive_c/users/user/AppData/Local/mygame/User Data/Crashpad/reports/SomeFile"
HUNT='hunt --hidden --first --exact SomeFile ~/';
FD='fd --hidden --no-ignore --glob --color=never --max-results=1 SomeFile ~/';
FIND='find ~/ -name SomeFile -print -quit 2>/dev/null';
LOCATE='locate -n 1 -A SomeFile'
hyperfine --warmup 1 --ignore-failure "$HUNT" "$FD" "$FIND" "$LOCATE";

