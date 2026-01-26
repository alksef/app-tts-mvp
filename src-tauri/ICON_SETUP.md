# Icon Setup for TTS Application

The Tauri build requires icon files. Follow these steps to set them up on Windows.

## Quick Method (Recommended)

1. Install the Tauri CLI:
   ```cmd
   npm install -g @tauri-apps/cli
   ```

2. Create a simple 1024x1024 PNG icon (you can use any image editor or online tool):
   - Use a purple background (#800080)
   - Add a white "T" in the center
   - Save as `source-icon.png` in the project root

3. Generate all required icons:
   ```cmd
   cd C:\RustProjects\app-tts
   tauri icon source-icon.png
   ```

This will create all required icon files in `src-tauri/icons/`.

## Alternative Method (ImageMagick)

If you have ImageMagick installed:

1. Create a simple PNG icon first
2. Convert to ICO:
   ```cmd
   magick convert 32x32.png icons\icon.ico
   ```

## PowerShell Script (Included)

Run the included PowerShell script to create a basic PNG:
```cmd
cd C:\RustProjects\app-tts\src-tauri
powershell -ExecutionPolicy Bypass -File create-icon.ps1
```

Then convert the resulting `32x32.png` to `icon.ico`.

## Manual Icon Creation

If you want to create icons manually, you need these files in `src-tauri/icons/`:
- `32x32.png` - 32x32 PNG icon
- `128x128.png` - 128x128 PNG icon
- `128x128@2x.png` - 256x256 PNG icon (2x resolution)
- `icon.ico` - Windows ICO file
- `icon.icns` - macOS ICNS file (optional, for macOS builds)

## Testing Without Icons

For development testing, you can temporarily disable icon requirements by editing `tauri.conf.json`:
```json
"bundle": {
  "icon": []
}
```

However, for production builds, proper icons are required.

## After Setting Up Icons

Once icons are in place, the build should work:
```cmd
npm run tauri:dev
```
