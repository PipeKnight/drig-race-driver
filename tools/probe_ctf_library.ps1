# Read-only bundled reference-library operation, using existing Windows .NET.
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskTool=Join-Path $taskRoot 'local/tools/ego-ctf-4.2/Ego CTF Editor'
$taskArchive=Join-Path $taskRoot 'local/tools/ego-ctf-4.2/official.7z'
if((Get-FileHash -LiteralPath $taskArchive).Hash.ToLower() -ne '751ac98af1389994a3eccc6fea1729f49ae17ed1ea8eed4f292553a73ce4d439'){throw 'Official archive local identity mismatch'}
$taskOut=Join-Path $taskRoot 'local/behavior/ctf-library-probe.json'
if(Test-Path -LiteralPath $taskOut){throw 'Refusing to replace evidence'}
Add-Type -Path (Join-Path $taskTool 'MiscUtil.dll')
Add-Type -Path (Join-Path $taskTool 'EgoEngineLibrary.dll')
$taskSchema=[IO.File]::OpenRead((Join-Path $taskTool 'ctfSchemaGrid.xml'))
$taskPage=New-Object EgoEngineLibrary.Vehicle.CtfEditorGamePage($taskSchema,([EgoEngineLibrary.Vehicle.CtfEditorMainTabs]::Grid))
$taskSource='C:/Program Files (x86)/Steam/steamapps/common/Grid/cars/lac/lac.csv'
$taskStream=[IO.File]::OpenRead($taskSource)
try{
 $taskCsv=New-Object EgoEngineLibrary.Vehicle.CsvFile($taskSource,$taskStream,$taskPage)
 $taskRecord=[pscustomobject]@{schema_fields=$taskPage.ctfEntryInfo.Count;line_index=$taskPage.lineIndex;files=$taskPage.files.Count;type=$taskCsv.GetType().FullName;source_sha256=(Get-FileHash -LiteralPath $taskSource).Hash.ToLower();assembly=$taskCsv.GetType().Assembly.FullName;loaded=$true}
 $taskRecord | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $taskOut -Encoding UTF8
 $taskRecord | ConvertTo-Json -Depth 5
}finally{$taskStream.Dispose()}
