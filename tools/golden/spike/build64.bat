@echo off
call "<Visual Studio>\VC\Auxiliary\Build\vcvars64.bat" >NUL || exit /b 1
set SP=<work folder>
set DT=%SP%\ext\DirectXTex\DirectXTex
if not exist "%SP%\bin64" mkdir "%SP%\bin64"
cl /nologo /O2 /EHsc /std:c++17 /D_CRT_SECURE_NO_WARNINGS /I"%DT%" "%SP%\work\dxtex_enc.cpp" "%DT%\BC.cpp" /Fe"%SP%\bin64\dxtex_enc.exe" /Fo"%SP%\bin64\\" || exit /b 1
echo built
