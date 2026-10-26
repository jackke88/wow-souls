param(
    [Parameter(Mandatory=$true)][string]$Binary,
    [Parameter(Mandatory=$true)][string]$WowData,
    [string[]]$Cases = @('tutorial','tutorial-far','westfall-near','ordinary-far','tutorial-gameplay')
)
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskExe = (Resolve-Path -LiteralPath $Binary).Path
$taskOut = Join-Path $taskRoot ('cache/soul-guidance-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $taskOut | Out-Null
$taskSaved = @{}
foreach ($taskVar in Get-ChildItem Env:WOWSOULS_*) { $taskSaved[$taskVar.Name] = $taskVar.Value }
foreach ($taskName in @('WOW_DATA','LOCALAPPDATA','RUST_LOG')) { $taskSaved[$taskName] = [Environment]::GetEnvironmentVariable($taskName) }
try {
    Get-ChildItem Env:WOWSOULS_* | Remove-Item
    $env:WOW_DATA = (Resolve-Path -LiteralPath $WowData).Path
    $env:RUST_LOG = 'warn,wow_souls=info'
    $env:WOWSOULS_WORLD = '1'
    $env:WOWSOULS_WIN = '1440x900'
    $env:WOWSOULS_AUDIO = 'null'
    $env:WOWSOULS_SPOKEN = '0'
    $env:WOWSOULS_GFX = 'classic'
    $env:WOWSOULS_HOUR = '13'
    $env:WOWSOULS_DAY_MINUTES = '0'
    $env:WOWSOULS_COMPASS_TRACE = '1'
    $env:WOWSOULS_OBSERVE_CLEAN = '1'
    # Stand 2.2 yd in front of Northshire's DB memorial, with a view of both the
    # stone and its overhead sign. Camera offsets use Bevy axes; anchors use WoW.
    $taskYaw = 5.82585
    $taskNorth = [math]::Cos($taskYaw)
    $taskEast = [math]::Sin($taskYaw)
    $taskCulture = [Globalization.CultureInfo]::InvariantCulture
    $taskAt = (((-8946.23 - $taskNorth * 2.2),(-183.477 - $taskEast * 2.2),79.9953) |
        ForEach-Object { $_.ToString('R', $taskCulture) }) -join ','
    $taskView = ((($taskEast * 12 + $taskNorth * 0.7),3.8,($taskNorth * 12 - $taskEast * 0.7)) |
        ForEach-Object { $_.ToString('R', $taskCulture) }) -join ','
    $taskCases = @(
        [pscustomobject]@{Name='tutorial'; At=$taskAt; View=$taskView; Quest=$true; Script='20:talk'; Shots='16,24'; Seconds=29},
        [pscustomobject]@{Name='tutorial-far'; At='-9090,-145,82'; View='-4,4,10'; Quest=$true; Script=''; Shots='16'; Seconds=22},
        [pscustomobject]@{Name='westfall-near'; At='-10562,1204.8,31.5'; View='7,5,9'; Quest=$false; Script=''; Shots='18'; Seconds=24},
        [pscustomobject]@{Name='ordinary-far'; At='-9491,165.021,61.641'; View='0,4,9'; Quest=$false; Script=''; Shots='16'; Seconds=22},
        [pscustomobject]@{Name='tutorial-gameplay'; At=$taskAt; View=''; Quest=$true; Script=''; Shots='16'; Seconds=22}
    )
    foreach ($taskCase in $taskCases | Where-Object { $_.Name -in $Cases }) {
        $env:LOCALAPPDATA = Join-Path $taskOut ('saves-' + $taskCase.Name)
        $env:WOWSOULS_AT = $taskCase.At
        if ($taskCase.View) { $env:WOWSOULS_OBSERVE = $taskCase.View }
        else { Remove-Item Env:WOWSOULS_OBSERVE -ErrorAction SilentlyContinue }
        $env:WOWSOULS_CHECK = $taskCase.Seconds.ToString()
        $env:WOWSOULS_SHOT_AT = $taskCase.Shots
        $env:WOWSOULS_SHOT = Join-Path $taskOut ($taskCase.Name + '.png')
        $env:WOWSOULS_QUESTS_SCRIPT = $taskCase.Script
        $env:WOWSOULS_QUESTS_SAVE = Join-Path $taskOut ($taskCase.Name + '-quests.txt')
        $env:WOWSOULS_BIND_SAVE = Join-Path $taskOut ($taskCase.Name + '-binding.txt')
        $taskSave = "wsquests 1`nname`tGuidancecheck`nxp`t0`nmoney`t0`n"
        if ($taskCase.Quest) { $taskSave += "quest`t9000101`tincomplete`t0 0 0 0`t0 0 0 0`n" }
        [IO.File]::WriteAllText($env:WOWSOULS_QUESTS_SAVE, $taskSave)
        $taskStdout = Join-Path $taskOut ($taskCase.Name + '.log')
        $taskStderr = Join-Path $taskOut ($taskCase.Name + '.stderr.log')
        $taskLaunch = @{FilePath=$taskExe; WorkingDirectory=$taskRoot; WindowStyle='Hidden'; RedirectStandardOutput=$taskStdout; RedirectStandardError=$taskStderr}
        $taskProcess = Start-Process @taskLaunch -PassThru
        if (-not $taskProcess.WaitForExit(60000)) { $taskProcess.Kill(); throw "Timed out: $($taskCase.Name); $taskOut" }
        $taskProcess.WaitForExit()
        $taskLog = [IO.File]::ReadAllText($taskStdout) + [IO.File]::ReadAllText($taskStderr)
        if ($taskProcess.ExitCode -ne 0 -or $taskLog -notmatch 'WOWSOULS_CHECK ok' -or
            $taskLog -match '(panicked|WOWSOULS_CHECK fault|ERROR bevy_asset::server)') {
            throw "Runtime failed: $($taskCase.Name); $taskOut"
        }
        switch ($taskCase.Name) {
            'tutorial' {
                foreach ($taskExpected in @('tutorial question mark over memorial 87047',
                    'removed tutorial question mark from memorial 87047',
                    'Return\(6774\)', 'Memorial \{ site: 87047, tutorial: false, bound: true \}')) {
                    if ($taskLog -notmatch $taskExpected) { throw "Missing $taskExpected; $taskOut" }
                }
                if ([IO.File]::ReadAllText($env:WOWSOULS_QUESTS_SAVE) -notmatch 'quest\s+9000101\s+complete') { throw "Tutorial did not complete; $taskOut" }
                if ([IO.File]::ReadAllText($env:WOWSOULS_BIND_SAVE) -notmatch 'site 87047') { throw "Binding was not saved; $taskOut" }
            }
            'tutorial-far' {
                if ($taskLog -notmatch 'Memorial \{ site: 87047, tutorial: true, bound: false \}.*distance 14[0-9]') { throw "Distant tutorial guidance is absent; $taskOut" }
            }
            'westfall-near' {
                if ($taskLog -notmatch 'Memorial \{ site: 40524, tutorial: false, bound: false \}' -or
                    $taskLog -match 'tutorial question mark over memorial') { throw "Quest-free Westfall discovery failed; $taskOut" }
            }
            'ordinary-far' {
                if ($taskLog -match 'compass: Memorial' -or $taskLog -notmatch '0 targets / 0 groups') { throw "Distant ordinary graveyard was revealed; $taskOut" }
            }
            'tutorial-gameplay' {
                if ($taskLog -notmatch 'tutorial question mark over memorial 87047') { throw "Tutorial gameplay marker is absent; $taskOut" }
            }
        }
        Write-Output "PASS: $($taskCase.Name), isolated saves and camera-relative compass."
    }
    Write-Output "Evidence: $taskOut"
} finally {
    Get-ChildItem Env:WOWSOULS_* | Remove-Item
    foreach ($taskName in $taskSaved.Keys) { [Environment]::SetEnvironmentVariable($taskName,$taskSaved[$taskName]) }
}
