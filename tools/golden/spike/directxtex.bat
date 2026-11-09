@echo off
rem Shared by the encoder builds: call "%~dp0directxtex.bat" after setup.bat.
rem Clones DirectXTex into %WORK%\ext\DirectXTex unless it is there, and sets DT to its library folder.
rem The tag is pinned because the comparison results depend on DirectXTex's BC3 encoder.
set "DIRECTXTEX_TAG=may2026"
set "DT=%WORK%\ext\DirectXTex\DirectXTex"
if exist "%DT%\BC.cpp" exit /b 0
git -c advice.detachedHead=false clone --quiet --depth 1 --branch %DIRECTXTEX_TAG% https://github.com/microsoft/DirectXTex "%WORK%\ext\DirectXTex" || exit /b 1
