param(
    [Parameter(Mandatory=$true)][string]$Binary,
    [Parameter(Mandatory=$true)][string]$WowData,
    [string[]]$Cases = @('sentinel','blanchy','longshore','dust')
)
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskExe = (Resolve-Path -LiteralPath $Binary).Path
$taskOut = Join-Path $taskRoot ('cache/westfall-runtime-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $taskOut | Out-Null
$taskSaved = @{}
foreach ($taskVar in Get-ChildItem Env:WOWSOULS_*) { $taskSaved[$taskVar.Name] = $taskVar.Value }
foreach ($taskName in @('WOW_DATA','LOCALAPPDATA','RUST_LOG')) { $taskSaved[$taskName] = [Environment]::GetEnvironmentVariable($taskName) }
try {
    Get-ChildItem Env:WOWSOULS_* | Remove-Item
    $env:WOW_DATA = (Resolve-Path -LiteralPath $WowData).Path
    $env:RUST_LOG = 'warn,wow_souls=info'
    $env:WOWSOULS_WORLD = '1'
    $env:WOWSOULS_DENSITY = '1'
    $env:WOWSOULS_WIN = '1280x800'
    $env:WOWSOULS_AUDIO = 'null'
    $env:WOWSOULS_SPOKEN = '0'
    $env:WOWSOULS_GFX = 'classic'
    $env:WOWSOULS_HOUR = '13'
    $env:WOWSOULS_DAY_MINUTES = '0'
    $taskCases = @(
        [pscustomobject]@{Name='sentinel'; At='-10511,1047,60.7'; Entry=234; Seconds=24; Script=''; Map='Westfall'},
        [pscustomobject]@{Name='blanchy'; At='-9854.8,918,31.1'; Entry=238; Seconds=40; Script='10:xp:100000,16:talk,18:confirm,20:confirm,22:back,24:give:1528x8,28:talk,30:confirm,32:confirm,34:confirm'; Map='Westfall'},
        [pscustomobject]@{Name='longshore'; At='-9650,1355,10.6'; Entry=830; Seconds=24; Script='10:xp:100000'; Map='Westfall'},
        # Pool 10003 enables eight of sixteen sites; GUID 48874 is in the deterministic active set.
        [pscustomobject]@{Name='dust'; At='-10230,890,42'; Entry=832; Seconds=24; Script='2:xp:100000'; Map='Westfall'}
    )
    foreach ($taskCase in $taskCases | Where-Object { $_.Name -in $Cases }) {
        $env:LOCALAPPDATA = Join-Path $taskOut ('saves-' + $taskCase.Name)
        $env:WOWSOULS_AT = $taskCase.At
        $env:WOWSOULS_CHECK = ($taskCase.Seconds + 8).ToString()
        $env:WOWSOULS_SHOT_AT = $taskCase.Seconds.ToString()
        $env:WOWSOULS_SHOT = Join-Path $taskOut ($taskCase.Name + '.png')
        $env:WOWSOULS_QUESTS_SCRIPT = $taskCase.Script
        $env:WOWSOULS_QUESTS_SAVE = Join-Path $taskOut ($taskCase.Name + '-quests.txt')
        if ($taskCase.Name -eq 'sentinel') { $env:WOWSOULS_MAP_SCRIPT='18:open' }
        else { Remove-Item Env:WOWSOULS_MAP_SCRIPT -ErrorAction SilentlyContinue }
        $taskStdout = Join-Path $taskOut ($taskCase.Name + '.log')
        $taskStderr = Join-Path $taskOut ($taskCase.Name + '.stderr.log')
        $taskLaunch = @{FilePath=$taskExe; WorkingDirectory=$taskRoot; WindowStyle='Hidden'; RedirectStandardOutput=$taskStdout; RedirectStandardError=$taskStderr}
        $taskProcess = Start-Process @taskLaunch -PassThru
        if (-not $taskProcess.WaitForExit(65000)) { $taskProcess.Kill(); throw "Timed out: $($taskCase.Name); $taskOut" }
        $taskProcess.WaitForExit()
        $taskLog = [IO.File]::ReadAllText($taskStdout) + [IO.File]::ReadAllText($taskStderr)
        if ($taskProcess.ExitCode -ne 0 -or $taskLog -notmatch 'WOWSOULS_CHECK ok' -or
            $taskLog -match '(panicked|WOWSOULS_CHECK fault|ERROR bevy_asset::server)') {
            throw "Runtime failed: $($taskCase.Name); $taskOut"
        }
        if ($taskLog -notmatch ('ring: \+ .* entry ' + $taskCase.Entry + ' ')) {
            throw "Expected NPC $($taskCase.Entry) was not placed: $($taskCase.Name); $taskOut"
        }
        if ($taskLog -notmatch ('map: switched to ' + $taskCase.Map)) {
            throw "Area map did not switch: $($taskCase.Name); $taskOut"
        }
        if ($taskCase.Name -eq 'blanchy') {
            $taskSave = [IO.File]::ReadAllText($env:WOWSOULS_QUESTS_SAVE)
            if ($taskSave -notmatch 'quest\s+151\s+rewarded') { throw "Poor Old Blanchy was not rewarded; $taskOut" }
        }
        Write-Output "PASS: $($taskCase.Name), NPC $($taskCase.Entry), native Westfall map and isolated saves."
    }
    Write-Output "Evidence: $taskOut"
} finally {
    Get-ChildItem Env:WOWSOULS_* | Remove-Item
    foreach ($taskName in $taskSaved.Keys) { [Environment]::SetEnvironmentVariable($taskName,$taskSaved[$taskName]) }
}
