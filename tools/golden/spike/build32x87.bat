@echo off
rem 32-bit x87 builds of the DirectXTex BC3 harness, one per /fp model, to test whether the DLL's
rem 1-step endpoint differences come from x87 floating point (the VS2010 x86 default).
rem usage: build32x87.bat [work folder]
setlocal
call "%~dp0setup.bat" 32 %1 || exit /b 1
call "%~dp0directxtex.bat" || exit /b 1
for %%V in (precise fast) do (
  if not exist "%WORK%\bin32x87_%%V" mkdir "%WORK%\bin32x87_%%V" || exit /b 1
  cl /nologo /O2 /EHsc /std:c++17 /arch:IA32 /fp:%%V /D_XM_NO_INTRINSICS_ /D_CRT_SECURE_NO_WARNINGS /I"%DT%" "%~dp0dxtex_enc.cpp" "%DT%\BC.cpp" /Fe"%WORK%\bin32x87_%%V\dxtex_enc.exe" /Fo"%WORK%\bin32x87_%%V\\" || exit /b 1
)
echo built "%WORK%\bin32x87_precise" "%WORK%\bin32x87_fast"
