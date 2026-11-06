@echo off
call "<Visual Studio>\VC\Auxiliary\Build\vcvars32.bat" >NUL || exit /b 1
set SP=<work folder>
if not exist "%SP%\bin32" mkdir "%SP%\bin32"
cl /nologo /O2 /D_CRT_SECURE_NO_WARNINGS "%SP%\work\dllharness.c" /Fe"%SP%\bin32\dllharness.exe" /Fo"%SP%\bin32\\" || exit /b 1
copy /Y "<repository>\AuroraAssetEditor\AuroraAsset.dll" "%SP%\bin32\" >NUL
copy /Y "<repository>\AuroraAssetEditor\msvcr100.dll" "%SP%\bin32\" >NUL
echo built
