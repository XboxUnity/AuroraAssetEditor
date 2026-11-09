@echo off
rem 32-bit harness around AuroraAsset.dll, with the DLL and its runtime copied next to it.
rem usage: build32.bat [work folder]
setlocal
call "%~dp0setup.bat" 32 %1 || exit /b 1
set "OUT=%WORK%\bin32"
if not exist "%OUT%" mkdir "%OUT%" || exit /b 1
cl /nologo /O2 /D_CRT_SECURE_NO_WARNINGS "%~dp0dllharness.c" /Fe"%OUT%\dllharness.exe" /Fo"%OUT%\\" || exit /b 1
copy /Y "%~dp0..\dll\AuroraAsset.dll" "%OUT%\" >nul || exit /b 1
copy /Y "%~dp0..\dll\msvcr100.dll" "%OUT%\" >nul || exit /b 1
echo built "%OUT%"
