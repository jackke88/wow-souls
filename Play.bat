@echo off
rem WoW Souls: builds the game and its animation builder (the first time takes 10-20 minutes), then starts it.
rem Extra arguments go to the game: Play.bat --choose-wow  or  Play.bat --setup-animations
cd /d "%~dp0"
where cargo >nul 2>nul || (
    echo Rust is not installed. Get it from https://rustup.rs and run Play.bat again.
    pause
    exit /b 1
)
echo Building WoW Souls...
cargo build --profile ship -p wow-souls --no-default-features || goto :failed
cargo build --release --manifest-path crates\dsr-cache\Cargo.toml || goto :failed
start "" "%~dp0target\ship\wow-souls.exe" %*
exit /b 0

:failed
echo.
echo The build failed; the messages above say why.
pause
exit /b 1
