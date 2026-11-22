@echo off
chcp 65001 >nul
echo ==========================================
echo   DANG BUILD S75 LED STUDIO (.EXE) ...
echo ==========================================
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [LOI] May chua cai Rust/Cargo!
    echo Tai Rust tai: https://rustup.rs/
    echo Hoac push thu muc nay len GitHub de GitHub Actions tu build file .exe cho ban.
    pause
    exit /b 1
)

cargo build --release
if %errorlevel% neq 0 (
    echo [LOI] Build that bai. Kiem tra lai thong bao o tren.
    pause
    exit /b 1
)

copy /Y target\release\s75_led_studio.exe s75_led_studio.exe >nul
echo.
echo [THANH CONG] File chay da san sang tai: s75_led_studio.exe
pause
