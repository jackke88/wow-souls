@echo off
rem WoW Souls showcase tour: a teleporter between every rare and quest creature, for filming.
rem   PageDown = next stop   PageUp = previous   Home = first (Garrick)   End = the same stop again
rem A jump brings the creature back to life, stands you in front of it and heals you. Stop 1 to 16 are
rem the rares (Garrick to Hogger), then the other quest creatures by level.
rem The tour plays at level 12 and keeps its kills and quest progress in temporary files, so your
rem character's saved quests, XP and slain rares are not touched.
cd /d "%~dp0"
echo Building WoW Souls (a few minutes if the code changed)...
cargo build --release -p wow-souls || (
    echo The build failed; the messages above say why.
    pause
    exit /b 1
)
copy /y "tools\tour\quests-level12.txt" "%TEMP%\wowsouls-tour-quests.txt" >nul
type nul > "%TEMP%\wowsouls-tour-world-state.txt"
set WOWSOULS_TOUR=1
set WOWSOULS_QUESTS_SAVE=%TEMP%\wowsouls-tour-quests.txt
set WOWSOULS_WORLD_STATE=%TEMP%\wowsouls-tour-world-state.txt
start "" "%~dp0target\release\wow-souls.exe" %*
exit /b 0
