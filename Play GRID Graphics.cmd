@echo off
setlocal
pushd "%~dp0"
if errorlevel 1 exit /b 1
set "GRID_GRAPHICS_EXE=target\release\grid-app.exe"
if exist "local\graphics\grid-graphics-app.exe" set "GRID_GRAPHICS_EXE=local\graphics\grid-graphics-app.exe"
set "GRID_GRAPHICS_ROOT=C:\Program Files (x86)\Steam\steamapps\common\Grid"
if not "%~1"=="" set "GRID_GRAPHICS_ROOT=%~1"
set "GRID_GRAPHICS_RESOLUTION=1920x1080"
if not "%~2"=="" set "GRID_GRAPHICS_RESOLUTION=%~2"
set "GRID_GRAPHICS_REFLECTIONS="
if /I "%~3"=="probe" set "GRID_GRAPHICS_REFLECTIONS=--reflections probe --probe-update moving --probe-size 1024"
if /I "%~3"=="probe-wheels" set "GRID_GRAPHICS_REFLECTIONS=--reflections probe --probe-update moving --probe-size 1024"
set "GRID_GRAPHICS_WHEELS="
if /I "%~3"=="wheels" set "GRID_GRAPHICS_WHEELS=--wheel-shutter 8.333 --wheel-geometry-shutter 8.333 --wheel-exposure adaptive --wheel-filter-floor 0.25 --wheel-max-arc 18"
if /I "%~3"=="probe-wheels" set "GRID_GRAPHICS_WHEELS=--wheel-shutter 8.333 --wheel-geometry-shutter 8.333 --wheel-exposure adaptive --wheel-filter-floor 0.25 --wheel-max-arc 18"
set "GRID_GRAPHICS_BODY_SPECULAR=--body-specular source-base"
if /I "%~3"=="no-body-specular" set "GRID_GRAPHICS_BODY_SPECULAR=--body-specular off"
set "GRID_GRAPHICS_BODY_OCCLUSION=--body-occlusion source-ambient"
if /I "%~3"=="no-body-occlusion" set "GRID_GRAPHICS_BODY_OCCLUSION=--body-occlusion off"
set "GRID_GRAPHICS_NUMBERS=--numbers source"
if /I "%~3"=="no-numbers" set "GRID_GRAPHICS_NUMBERS=--numbers off"
set "GRID_GRAPHICS_SUN=--sun source"
if /I "%~3"=="sun-authored" set "GRID_GRAPHICS_SUN=--sun authored"
set "GRID_GRAPHICS_TREES=--trees coverage"
if /I "%~3"=="no-trees" set "GRID_GRAPHICS_TREES="
if /I "%~3"=="trees" set "GRID_GRAPHICS_TREES=--trees coverage"
set "GRID_GRAPHICS_HAZE="
if /I "%~3"=="atmosphere" (
  set "GRID_GRAPHICS_TREES=--trees coverage"
  set "GRID_GRAPHICS_HAZE=--haze source"
)
set "GRID_GRAPHICS_TEXTURES="
if /I "%~3"=="textures" (
  if not exist "local\graphics\texture-pilot02\buildings-two.overrides.tsv" (
    echo Private texture pilot is missing. See docs\TEXTURE_OVERRIDES.md.
    popd
    pause
    exit /b 3
  )
  set "GRID_GRAPHICS_TEXTURES=--texture-overrides local\graphics\texture-pilot02\buildings-two.overrides.tsv"
)
if not exist "%GRID_GRAPHICS_EXE%" (
  echo Build this graphics checkout first: cargo build -p grid-app --release --offline
  popd
  pause
  exit /b 2
)
if not exist "local\graphics" mkdir "local\graphics"
rem Query once: old verified or other-agent executables remain usable.
set "GRID_GRAPHICS_CAPABILITIES=local\graphics\capabilities-%RANDOM%-%RANDOM%.txt"
"%GRID_GRAPHICS_EXE%" --help > "%GRID_GRAPHICS_CAPABILITIES%" 2>&1
"%SystemRoot%\System32\findstr.exe" /L /C:"--numbers source|off" "%GRID_GRAPHICS_CAPABILITIES%" >nul
if errorlevel 1 (
  set "GRID_GRAPHICS_NUMBERS="
  echo This executable predates race numbers; using its supported graphics controls.
)
"%SystemRoot%\System32\findstr.exe" /L /C:"--body-occlusion off|source-ambient" "%GRID_GRAPHICS_CAPABILITIES%" >nul
if errorlevel 1 (
  set "GRID_GRAPHICS_BODY_OCCLUSION="
  echo This executable predates paint occlusion; using its supported graphics controls.
)
"%SystemRoot%\System32\findstr.exe" /L /C:"--body-specular off|source-base|white-control" "%GRID_GRAPHICS_CAPABILITIES%" >nul
if errorlevel 1 (
  set "GRID_GRAPHICS_BODY_SPECULAR="
  echo Paint specular maps are unavailable in this build; using its supported graphics controls.
)
set "GRID_GRAPHICS_CONTACTS="
if exist "local\body-contact-directions.bin" set "GRID_GRAPHICS_CONTACTS=--body-contacts local\body-contact-directions.bin"
if defined GRID_GRAPHICS_CONTACTS if exist "local\body-impact-parameters.bin" set "GRID_GRAPHICS_CONTACTS=%GRID_GRAPHICS_CONTACTS% --body-events local\body-impact-parameters.bin"
if defined GRID_GRAPHICS_CONTACTS if exist "local\deformation-parameters.bin" set "GRID_GRAPHICS_CONTACTS=%GRID_GRAPHICS_CONTACTS% --deformation local\deformation-parameters.bin"
if /I "%~3"=="body" (
  if not exist "local\graphics\materials-livery01\body-realesrgan8k01.overrides.tsv" (
    echo Private Lacetti body texture pilot is missing. See docs\TEXTURE_OVERRIDES.md.
    popd
    pause
    exit /b 3
  )
  set "GRID_GRAPHICS_TEXTURES=--texture-overrides local\graphics\materials-livery01\body-realesrgan8k01.overrides.tsv"
)
set "GRID_GRAPHICS_LOG=local\graphics\manual-%RANDOM%-%RANDOM%.log"
echo Experimental graphics branch - source sky lighting, layered car/track materials and smooth tyres.
echo Arrows/WASD drive; Space handbrake; Escape/P pause menu; R restart.
echo Resolution %GRID_GRAPHICS_RESOLUTION%; maximize the window for your display.
if defined GRID_GRAPHICS_REFLECTIONS echo Environment reflections enabled.
if defined GRID_GRAPHICS_WHEELS echo Experimental wheel filtering enabled; geometry filtering is player-only.
if defined GRID_GRAPHICS_TREES echo Experimental source forest enabled; wind and foliage lighting remain approximate.
if defined GRID_GRAPHICS_HAZE echo Experimental source distance haze enabled.
if defined GRID_GRAPHICS_TEXTURES echo Experimental high-resolution textures enabled.
if defined GRID_GRAPHICS_BODY_OCCLUSION echo Source paint occlusion control: %GRID_GRAPHICS_BODY_OCCLUSION%
if defined GRID_GRAPHICS_BODY_SPECULAR echo Source paint reflection control: %GRID_GRAPHICS_BODY_SPECULAR%
echo Graphics executable: %GRID_GRAPHICS_EXE%
echo Private log: %GRID_GRAPHICS_LOG%
"%GRID_GRAPHICS_EXE%" --drive --route tracks/europe/donington/route_0 --car lac --graphics enhanced --ibl source --resolution "%GRID_GRAPHICS_RESOLUTION%" --game-root "%GRID_GRAPHICS_ROOT%" %GRID_GRAPHICS_CONTACTS% %GRID_GRAPHICS_REFLECTIONS% %GRID_GRAPHICS_WHEELS% %GRID_GRAPHICS_TEXTURES% %GRID_GRAPHICS_TREES% %GRID_GRAPHICS_HAZE% %GRID_GRAPHICS_SUN% %GRID_GRAPHICS_NUMBERS% %GRID_GRAPHICS_BODY_OCCLUSION% %GRID_GRAPHICS_BODY_SPECULAR% > "%GRID_GRAPHICS_LOG%" 2>&1
set "GRID_GRAPHICS_EXIT=%ERRORLEVEL%"
echo Exited with code %GRID_GRAPHICS_EXIT%.
popd
pause
exit /b %GRID_GRAPHICS_EXIT%
