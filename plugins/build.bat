@echo off
cargo build --release
if %ERRORLEVEL% NEQ 0 exit /b %ERRORLEVEL%
copy target\release\file_logger_plugin.dll ..\src-tauri\plugins\ /Y
echo Plugin built and copied to src-tauri/plugins/
