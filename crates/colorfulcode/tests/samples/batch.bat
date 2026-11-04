@echo off
REM Build the project and copy the output.
setlocal enabledelayedexpansion

set "OUT_DIR=%~dp0dist"
set /a COUNT=0
:: Parse arguments
if "%1"=="" goto usage
if /i "%1"=="clean" (
    rmdir /s /q "%OUT_DIR%" 2>nul
    echo Cleaned %OUT_DIR%
    goto :eof
)

if not exist "%OUT_DIR%" mkdir "%OUT_DIR%"

for %%f in (src\*.c) do (
    echo Compiling %%~nf
    cl /nologo /c "%%f" /Fo"%OUT_DIR%\%%~nf.obj" || goto failed
    set /a COUNT+=1
)
echo Built !COUNT! files for %PROCESSOR_ARCHITECTURE:~0,3%.
call :log "done"
exit /b 0

:usage
echo Usage: %~nx0 [build^|clean]
exit /b 1

:failed
echo Build failed with error %ERRORLEVEL% & exit /b %ERRORLEVEL%

:log
echo [%DATE% %TIME%] %~1 >> build.log
goto :eof
