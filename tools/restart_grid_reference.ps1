param(
 [Parameter(Mandatory=$true)][string]$SessionJson,
 [Parameter(Mandatory=$true)][string]$EvidenceDirectory,
 [int]$RecordedChildProcessId=0
)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$privateRoot=Join-Path $root 'local'
$out=[IO.Path]::GetFullPath($EvidenceDirectory)
if(-not $out.StartsWith($privateRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Evidence must remain in project local/' }
if(Test-Path -LiteralPath $out) { throw 'Evidence directory already exists' }
$session=Get-Content -LiteralPath $SessionJson -Raw | ConvertFrom-Json
$savedStart=[DateTimeOffset]$session.StartedUtc
$work='C:\Users\debros\Tools\grid-research\grid-compat'
$exe=Join-Path $work 'grid.exe'
$usedChild=$false
if($RecordedChildProcessId) {
 $child=Get-CimInstance Win32_Process -Filter ('ProcessId='+$RecordedChildProcessId)
 $delta=if($child) { ($child.CreationDate.ToUniversalTime()-$savedStart.UtcDateTime).TotalSeconds } else { -1 }
 if(-not $child -or $child.ParentProcessId -ne $session.Pid -or $child.ExecutablePath -ne $exe -or $delta -lt 0 -or $delta -gt 120) { throw 'Recorded child identity mismatch' }
 $savedStart=[DateTimeOffset]$child.CreationDate
 $current=Get-Process -Id $RecordedChildProcessId -ErrorAction Stop
 $usedChild=$true
} else { $current=Get-Process -Id $session.Pid -ErrorAction Stop }
if($current.ProcessName -ne 'grid' -or $current.Path -ne $exe -or [Math]::Abs(($current.StartTime.ToUniversalTime()-$savedStart.UtcDateTime).TotalSeconds) -gt 2) { throw 'Reference process identity mismatch' }
$hash=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLower()
if($hash -ne $session.ExeSHA256) { throw 'Reference executable hash mismatch' }
$settings=Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Codemasters\GRID'
New-Item -ItemType Directory -Path $out | Out-Null
Copy-Item -LiteralPath $settings -Destination (Join-Path $out 'user-before') -Recurse
Stop-Process -Id $current.Id
if(-not $current.WaitForExit(10000)) { throw 'Reference did not exit' }
$proc=Start-Process -FilePath $exe -WorkingDirectory $work -PassThru
$record=[PSCustomObject]@{Pid=$proc.Id;StartedUtc=$proc.StartTime.ToUniversalTime().ToString('o');ExeSHA256=$hash;Codepage=$session.Codepage;PriorPid=$current.Id;PriorRecordedParent=$session.Pid;UsedVerifiedChild=$usedChild;UserDirectoryEdited=$false;OriginalInstallEdited=$false}
$record | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $out 'session.json') -Encoding utf8
$record | ConvertTo-Json
