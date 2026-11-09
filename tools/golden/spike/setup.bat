@echo off
rem Shared by the build scripts: call "%~dp0setup.bat" <32|64> [work folder]
rem Sets WORK (default <repo>\target\golden-spike) and loads the MSVC environment for the target.
rem No labels: cmd can miss a goto label in a file with LF line endings.
set "WORK=%~2"
if "%WORK%"=="" set "WORK=%~dp0..\..\..\target\golden-spike"
for %%I in ("%WORK%") do set "WORK=%%~fI"

set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" (
  echo error: "%VSWHERE%" not found. Install Visual Studio 2017 or later with the C++ x86/x64 build tools. 1>&2
  exit /b 1
)
set "VSDIR="
rem The leading "call" keeps cmd from stripping the quotes around a path that contains "(x86)".
for /f "usebackq delims=" %%P in (`call "%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VSDIR=%%P"
if "%VSDIR%"=="" (
  echo error: vswhere found no Visual Studio with the C++ x86/x64 build tools. 1>&2
  exit /b 1
)
set "VCVARS=%VSDIR%\VC\Auxiliary\Build\vcvars%~1.bat"
if not exist "%VCVARS%" (
  echo error: "%VCVARS%" not found. 1>&2
  exit /b 1
)
call "%VCVARS%" >nul || exit /b 1
