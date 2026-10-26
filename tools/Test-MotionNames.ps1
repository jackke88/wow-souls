$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskLibrary = Join-Path $taskRoot 'anim/source'
$taskCount = 0
foreach ($taskRow in [IO.File]::ReadLines((Join-Path $taskRoot 'anim/provenance/motion-names.tsv'))) {
    if ($taskRow.StartsWith('#')) { continue }
    $taskFields = $taskRow -split '\t'
    $taskPath = Join-Path $taskLibrary $taskFields[1]
    $taskText = [IO.File]::ReadAllText($taskPath)
    $taskPayload = (@($taskText -split '\r?\n' | Where-Object { $_ -notmatch '^(#|name )' }) -join [char]10).TrimEnd([char]10)
    $taskHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskPayload)))
    if ($taskHash -ine $taskFields[2]) { throw "Motion payload changed: $($taskFields[1])" }
    if ([IO.Path]::GetFileNameWithoutExtension($taskPath) -match '^a\d{2}_\d{4}$') { throw "Raw runtime name: $taskPath" }
    $taskCount++
}
if ($taskCount -ne 1203) { throw "Expected 1203 preserved motion payloads, got $taskCount" }
Write-Output "PASS: all $taskCount numeric motion payloads are unchanged; project paths use readable names."
