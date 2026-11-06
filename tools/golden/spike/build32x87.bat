@echo off
rem Spike: 32-bit x87 builds of the DirectXTex BC3 harness, to test whether the DLL's
rem 1-step endpoint differences come from x87 floating point (VS2010 x86 default).
call "<Visual Studio>\VC\Auxiliary\Build\vcvars32.bat" >NUL || exit /b 1
set SP=<work folder>
set DT=%SP%\ext\DirectXTex\DirectXTex
for %%V in (precise fast) do (
  if not exist "%SP%\bin32x87_%%V" mkdir "%SP%\bin32x87_%%V"
  cl /nologo /O2 /EHsc /std:c++17 /arch:IA32 /fp:%%V /D_XM_NO_INTRINSICS_ /D_CRT_SECURE_NO_WARNINGS /I"%DT%" "%SP%\work\dxtex_enc.cpp" "%DT%\BC.cpp" /Fe"%SP%\bin32x87_%%V\dxtex_enc.exe" /Fo"%SP%\bin32x87_%%V\\" || exit /b 1
)
echo built
