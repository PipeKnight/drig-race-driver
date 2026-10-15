@echo off
setlocal
pushd "%~dp0"
if errorlevel 1 exit /b 1
set "GRID_GRAPHICS_ROOT=C:\Program Files (x86)\Steam\steamapps\common\Grid"
if not "%~1"=="" set "GRID_GRAPHICS_ROOT=%~1"
set "GRID_GRAPHICS_RESOLUTION=1920x1080"
if not "%~2"=="" set "GRID_GRAPHICS_RESOLUTION=%~2"
if not exist "target\release\grid-app.exe" (
  echo Build this graphics checkout first: cargo build -p grid-app --release --offline
  popd
  pause
  exit /b 2
)
if not exist "local\graphics" mkdir "local\graphics"
set "GRID_GRAPHICS_CONTACTS="
if exist "local\body-contact-directions.bin" set "GRID_GRAPHICS_CONTACTS=--body-contacts local\body-contact-directions.bin"
if defined GRID_GRAPHICS_CONTACTS if exist "local\body-impact-parameters.bin" set "GRID_GRAPHICS_CONTACTS=%GRID_GRAPHICS_CONTACTS% --body-events local\body-impact-parameters.bin"
if defined GRID_GRAPHICS_CONTACTS if exist "local\deformation-parameters.bin" set "GRID_GRAPHICS_CONTACTS=%GRID_GRAPHICS_CONTACTS% --deformation local\deformation-parameters.bin"
set "GRID_GRAPHICS_LOG=local\graphics\manual-%RANDOM%-%RANDOM%.log"
echo Experimental graphics branch - warm GRID-inspired lighting.
echo Arrows/WASD drive; Space handbrake; P pause; R reset; Escape exit.
echo Resolution %GRID_GRAPHICS_RESOLUTION%; maximize the window for your display.
echo Private log: %GRID_GRAPHICS_LOG%
"target\release\grid-app.exe" --drive --graphics enhanced --resolution "%GRID_GRAPHICS_RESOLUTION%" --game-root "%GRID_GRAPHICS_ROOT%" %GRID_GRAPHICS_CONTACTS% > "%GRID_GRAPHICS_LOG%" 2>&1
set "GRID_GRAPHICS_EXIT=%ERRORLEVEL%"
echo Exited with code %GRID_GRAPHICS_EXIT%.
popd
pause
exit /b %GRID_GRAPHICS_EXIT%
