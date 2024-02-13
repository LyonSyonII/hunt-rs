# Benchmark 1: Searching file in ~/
# File is in "/home/user/.wine/drive_c/users/user/AppData/Local/mygame/User Data/Crashpad/reports/SomeFile"
HUNT="hunt --hidden --first --exact SomeFile $HOME";
FD="fd --hidden --no-ignore --color=never --max-results=1 SomeFile $HOME";
FIND="find $HOME -name SomeFile -print -quit";
# LOCATE='locate -n 1 -A SomeFile'
hyperfine -N --warmup 2 --ignore-failure "$FIND"  \
   "$FD" \
   "$HUNT" \

