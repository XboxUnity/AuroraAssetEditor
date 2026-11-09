@echo off
rem 64-bit DirectXTex BC3 encoder harness (SSE floating point).
rem usage: build64.bat [work folder]
setlocal
call "%~dp0setup.bat" 64 %1 || exit /b 1
call "%~dp0directxtex.bat" || exit /b 1
set "OUT=%WORK%\bin64"
if not exist "%OUT%" mkdir "%OUT%" || exit /b 1
cl /nologo /O2 /EHsc /std:c++17 /D_CRT_SECURE_NO_WARNINGS /I"%DT%" "%~dp0dxtex_enc.cpp" "%DT%\BC.cpp" /Fe"%OUT%\dxtex_enc.exe" /Fo"%OUT%\\" || exit /b 1
echo built "%OUT%"
