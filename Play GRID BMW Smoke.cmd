@echo off
setlocal
pushd "%~dp0"
if errorlevel 1 exit /b 1
rem Maintained default-feature BMW/Istanbul smoke-order option; ordinary launchers stay unchanged.
rem Never inherit research switches or the owner's stored event/damage profile.
for /f "tokens=1 delims==" %%V in ('set GRID_ 2^>nul') do set "%%V="
for /f "tokens=1 delims==" %%V in ('set WGPU_ 2^>nul') do set "%%V="
set "DLSS_SDK="
set "WGPU_BACKEND=vulkan"
set "BMW_EXIT=2"
if not exist "target\release\grid-app.exe" (
  echo Build the ordinary prototype in this checkout first.
  goto finish
)
set "BMW_ROOT_ARGS="
if not "%~1"=="" set "BMW_ROOT_ARGS=--game-root "%~1""
set "BMW_SESSION=local\playtest\bmw-smoke-%RANDOM%-%RANDOM%-%RANDOM%"
set "BMW_EXIT=4"
if exist "%BMW_SESSION%" (
  echo The new session path already exists. Please run this launcher again.
  goto finish
)
mkdir "%BMW_SESSION%\data"
if errorlevel 1 goto finish
echo BMW / Istanbul: baseline smoke-order fix, keyboard driving. This option does not use DLAA.
echo Normal Play GRID defaults are unchanged. Choose Go To Race, then drive normally.
echo Space: handbrake. C: camera. R: restart. Escape/P: pause.
echo Private log: %BMW_SESSION%\game.log
"target\release\grid-app.exe" --drive --route tracks/europe/istanbul_park/route_0 --car bmw --graphics baseline --aa taa --reflections sky --input device=keyboard --lod LOD0_ --data-dir "%BMW_SESSION%\data" --car-transparent-sort off --smoke-order groups --skid-order off --smoke-ct off --smoke-boundary grouped-live --smoke-compositor off --ground-skid-pass off --resolution 1920x1080 %BMW_ROOT_ARGS% > "%BMW_SESSION%\game.log" 2>&1
set "BMW_EXIT=%ERRORLEVEL%"
> "%BMW_SESSION%\exit-code.txt" echo %BMW_EXIT%
echo Exited with code %BMW_EXIT%.
:finish
popd
pause
exit /b %BMW_EXIT%
