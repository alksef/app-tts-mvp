# PowerShell script to create a minimal icon.ico file for Tauri
# Run this on Windows: powershell -ExecutionPolicy Bypass -File create-icon.ps1

Add-Type -AssemblyName System.Drawing

# Create a 32x32 bitmap
$bmp = New-Object System.Drawing.Bitmap 32, 32
$g = [System.Drawing.Graphics]::FromImage($bmp)

# Draw purple background
$g.Clear([System.Drawing.Color]::FromArgb(128, 0, 128))

# Draw "T" text
$font = New-Object System.Drawing.Font('Arial', 18, [System.Drawing.FontStyle]::Bold)
$brush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
$stringFormat = New-Object System.Drawing.StringFormat
$stringFormat.Alignment = [System.Drawing.StringAlignment]::Center
$stringFormat.LineAlignment = [System.Drawing.StringAlignment]::Center
$g.DrawString('T', $font, $brush, 16, 16, $stringFormat)

# Save as PNG first (convert to ICO requires additional libraries)
$bmp.Save('icons\32x32.png', [System.Drawing.Imaging.ImageFormat]::Png)

# Cleanup
$g.Dispose()
$bmp.Dispose()

Write-Host "Created icons\32x32.png"
Write-Host ""
Write-Host "To create a proper icon.ico file, you have two options:"
Write-Host ""
Write-Host "Option 1: Use Tauri CLI (recommended)"
Write-Host "  1. Create a 1024x1024 source PNG"
Write-Host "  2. Run: npx tauri-icon source.png"
Write-Host ""
Write-Host "Option 2: Use ImageMagick"
Write-Host "  magick convert 32x32.png icon.ico"
Write-Host ""
Write-Host "For now, using 32x32.png as fallback..."
