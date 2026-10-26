param(
    [string]$Binary,
    [string]$WowData = $env:WOW_DATA
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$repoPrefix = $repo + [IO.Path]::DirectorySeparatorChar
Push-Location $repo
try {
    $hostLine = rustc -vV | Select-String '^host: '
    if ($LASTEXITCODE -ne 0 -or -not $hostLine) { throw 'Cannot identify Rust host' }
    $platform = $hostLine.ToString().Substring(6)
    $json = cargo metadata --offline --locked --format-version 1 --filter-platform $platform
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
    $metadata = $json | ConvertFrom-Json
    $local = @($metadata.packages | Where-Object { -not $_.source })
    foreach ($package in $local) {
        $path = [IO.Path]::GetFullPath($package.manifest_path)
        if (-not $path.StartsWith($repoPrefix, [StringComparison]::OrdinalIgnoreCase)) {
            throw "External local code dependency: $($package.name) at $path"
        }
        if ($package.name -like 'dsr-*') { throw "DSR package in production graph: $($package.name)" }
    }
    if ($metadata.workspace_members.Count -ne 3) { throw 'Unexpected production workspace members' }
    foreach ($relative in @('anim/source/inventory.tsv', 'data/world/elwynn-westfall.txt', 'data/world/westfall.txt',
        'data/world/northshire.txt', 'vendor/benilla/LICENSE-MIT', 'vendor/benilla/LICENSE-APACHE')) {
        if (-not (Test-Path -LiteralPath $relative -PathType Leaf)) { throw "Missing project asset: $relative" }
    }
    $tracked = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    git ls-files | ForEach-Object { [void]$tracked.Add($_) }
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect project file tracking' }
    foreach ($scope in @('crates/wow-souls', 'crates/wow-world', 'crates/wscl',
        'vendor/benilla', 'anim/source', 'data')) {
        foreach ($file in Get-ChildItem -LiteralPath $scope -Recurse -File) {
            $relative = $file.FullName.Substring($repoPrefix.Length).Replace('\', '/')
            if (-not $tracked.Contains($relative)) { throw "Required project file is untracked: $relative" }
        }
    }
    if (-not $tracked.Contains('docs/ENEMY-AI-MAPPING.tsv')) { throw 'Required AI test fixture is untracked' }
    $sumRoot = Join-Path $repo 'vendor/benilla'
    foreach ($row in Get-Content -LiteralPath (Join-Path $sumRoot 'SHA256SUMS')) {
        $parts = $row -split '  ', 2
        $actual = (Get-FileHash -LiteralPath (Join-Path $sumRoot $parts[1]) -Algorithm SHA256).Hash
        if ($actual -ine $parts[0]) { throw "Engine snapshot differs: $($parts[1]); review and update provenance" }
    }
    Write-Output "PASS: $($local.Count) local packages inside the repository; no DSR production dependency."
    if (-not $Binary) { return }
    $exe = (Resolve-Path -LiteralPath $Binary).Path
    if (-not $WowData -or -not (Test-Path -LiteralPath $WowData -PathType Container)) {
        throw 'Supply -WowData pointing to your WoW 1.12.1 Data folder'
    }
    $WowData = (Resolve-Path -LiteralPath $WowData).Path
    $out = Join-Path ([IO.Path]::GetTempPath()) ('wow-only-' + [guid]::NewGuid().ToString('N'))
    $package = Join-Path $out 'package'
    $cwd = Join-Path $out 'empty-working-directory'
    New-Item -ItemType Directory -Path $package, $cwd, (Join-Path $package 'anim') | Out-Null
    Copy-Item -LiteralPath $exe -Destination (Join-Path $package 'wow-souls.exe')
    Copy-Item -LiteralPath 'anim/source' -Destination (Join-Path $package 'anim') -Recurse
    Copy-Item -LiteralPath 'data' -Destination $package -Recurse
    if (Get-ChildItem -LiteralPath $package -Recurse -File -Filter '*.wscl') {
        throw 'Isolated package contains a binary extraction clip'
    }
    $saved = @{}
    $names = @('WOW_DATA', 'DSR_DIR', 'ER_DIR', 'LOCALAPPDATA', 'RUST_LOG')
    foreach ($entry in Get-ChildItem Env:WOWSOULS_*) { $names += $entry.Name }
    foreach ($name in $names | Select-Object -Unique) { $saved[$name] = [Environment]::GetEnvironmentVariable($name) }
    try {
        Get-ChildItem Env:WOWSOULS_* | Remove-Item
        $env:WOW_DATA = $WowData
        $env:DSR_DIR = Join-Path $out 'absent-dsr-install'
        $env:ER_DIR = Join-Path $out 'absent-er-install'
        $env:RUST_LOG = 'warn,wow_souls=info'
        $env:WOWSOULS_DSR_CACHE = Join-Path $out 'absent-dsr-cache'
        $env:WOWSOULS_CHECK = '20'
        $env:WOWSOULS_WIN = '960x540'
        $env:WOWSOULS_AUDIO = 'null'
        $env:WOWSOULS_GFX = 'classic'
        $env:WOWSOULS_GRASS = '0'
        $env:WOWSOULS_AT = '-8906,-136,82'
        $env:WOWSOULS_KOBOLD_AT = '-8901,-136,82'
        $env:WOWSOULS_KOBOLDS = '1'
        $env:WOWSOULS_WOLVES = '0'
        $env:WOWSOULS_THUGS = '0'
        $env:WOWSOULS_GARRICKS = '0'
        $env:WOWSOULS_SEED = '41'
        $env:WOWSOULS_HOUR = '12'
        $env:WOWSOULS_DAY_MINUTES = '0'
        foreach ($case in @('combat', 'retired-selectors', 'world')) {
            $env:LOCALAPPDATA = Join-Path $out ('saves-' + $case)
            $env:WOWSOULS_WORLD = if ($case -eq 'world') { '1' } else { '0' }
            $env:WOWSOULS_ATTACK = '3,3'
            $env:WOWSOULS_ROLL = '5,4'
            if ($case -eq 'retired-selectors') {
                $env:WOWSOULS_UI = 'legacy'
                $env:WOWSOULS_VITALS = 'legacy'
            } else {
                Remove-Item Env:WOWSOULS_UI, Env:WOWSOULS_VITALS -ErrorAction SilentlyContinue
            }
            $stdout = Join-Path $out ($case + '.stdout.log')
            $stderr = Join-Path $out ($case + '.stderr.log')
            $process = Start-Process -FilePath (Join-Path $package 'wow-souls.exe') -WorkingDirectory $cwd `
                -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
            if (-not $process.WaitForExit(45000)) {
                $process.Kill()
                throw "Isolated $case timed out; logs: $out"
            }
            $process.WaitForExit()
            $log = [IO.File]::ReadAllText($stdout) + [IO.File]::ReadAllText($stderr)
            foreach ($pattern in @('WOWSOULS_CHECK ok', 'weapons: loaded 19 families',
                'locomotion: 66 of the gait clips', 'reaction: 16 clips, 2 deaths',
                'hud vitals: native', 'settle: released')) {
                if ($log -notmatch [regex]::Escape($pattern)) { throw "Missing '$pattern' in $case; logs: $out" }
            }
            if ($log -notmatch 'voices: Spoken Player quest voices from .*data[/\\]voices') {
                throw "Bundled voice files were not found in $case; logs: $out"
            }
            if ($case -eq 'world' -and $log -notmatch 'world: \d+ spots .*from .*package[/\\]data[/\\]world[/\\]elwynn-westfall\.txt') {
                throw "Packaged world was not loaded; logs: $out"
            }
            if ($process.ExitCode -ne 0 -or $log -match '(?i)(panicked|unavailable|found no ground|no Dark Souls|no verified .*cache|ERROR bevy_asset::server|WOWSOULS_CHECK fault)') {
                throw "Isolated $case failed; logs: $out"
            }
            Write-Output "PASS: isolated $case with bundled project data, no binary clips and absent DSR/ER/cache paths."
        }
    } finally {
        Get-ChildItem Env:WOWSOULS_* | Remove-Item
        foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name, $saved[$name]) }
    }
    Write-Output "Evidence retained at $out"
} finally { Pop-Location }
