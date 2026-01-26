@echo off
REM Script to generate Tauri icons on Windows
REM Requires ImageMagick or you can use the tauri icon command

echo Generating TTS icons...
echo.

REM Option 1: Use Tauri CLI (recommended)
echo Option 1: Using Tauri CLI (requires source PNG)
echo Run: npx tauri-icon path-to-source.png
echo.

REM Option 2: Create minimal icons using PowerShell
echo Option 2: Creating minimal placeholder icons...
echo.

REM Create a simple 32x32 ICO file using PowerShell
powershell -Command "
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap 32, 32
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.Clear([System.Drawing.Color]::Purple)
$font = New-Object System.Drawing.Font('Arial', 14)
$brush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
$g.DrawString('T', $font, $brush, 8, 5)
$bmp.Save('icons\32x32.png', [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()
"

echo Icon files created in icons\ directory
echo You may need to install ImageMagick or use the Tauri CLI for better quality icons
echo.
echo To use Tauri CLI:
echo   1. Create a 1024x1024 PNG source icon
echo   2. Run: npm install -g @tauri-apps/cli
echo   3. Run: tauri icon source.png
echo.

pause
