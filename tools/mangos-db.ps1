# A throwaway MySQL over a COPY of the vMaNGOS world database, for reading spawns, NPCs and quests.
#
#   tools\mangos-db.ps1 start            copy the database once (cache\mangos\db), start mysqld on 127.0.0.1:3399
#   tools\mangos-db.ps1 stop             stop it
#   tools\mangos-db.ps1 query "<sql>"    run one statement and print the rows (tab separated, with headers)
#   tools\mangos-db.ps1 start -Refresh   recopy the database first
#
# The repack's own server is never started and its data directory is only read (Copy-Item).
# Everything lives under cache\mangos, which is gitignored with the rest of /cache.

param(
    [Parameter(Mandatory = $true, Position = 0)][ValidateSet('start', 'stop', 'query')][string]$Action,
    [Parameter(Position = 1)][string]$Sql = '',
    [switch]$Refresh
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$source = 'C:\Spel\MaNGOS\mysql5'
$work = Join-Path $repo 'cache\mangos\db'
$data = Join-Path $work 'data'
$mysql = Join-Path $source 'bin\mysql.exe'
$mysqld = Join-Path $source 'bin\mysqld.exe'
$port = 3399

function Invoke-Sql([string]$statement) {
    & $mysql --no-defaults --host=127.0.0.1 --port=$port --user=root --default-character-set=utf8 --batch -e $statement
}

function Test-Up {
    try { $null = Invoke-Sql 'select 1'; return $LASTEXITCODE -eq 0 } catch { return $false }
}

switch ($Action) {
    'start' {
        if (Test-Up) { Write-Host "mysqld already answers on $port"; return }
        if ($Refresh -and (Test-Path $data)) { Remove-Item $data -Recurse -Force }
        if (-not (Test-Path (Join-Path $data 'mangos'))) {
            New-Item -ItemType Directory -Force $data | Out-Null
            Copy-Item (Join-Path $source 'data\mangos') (Join-Path $data 'mangos') -Recurse
            Copy-Item (Join-Path $source 'data\mysql') (Join-Path $data 'mysql') -Recurse
        }
        $dataFwd = $data -replace '\\', '/'
        $sourceFwd = $source -replace '\\', '/'
        # MySQL 5.5.62 defaults secure_file_priv to NULL (INTO OUTFILE refused); allow it for the export directory only.
        $tsv = Join-Path $repo 'cache\mangos\tsv'
        New-Item -ItemType Directory -Force $tsv | Out-Null
        $tsvFwd = $tsv -replace '\\', '/'
        Start-Process -FilePath $mysqld -WindowStyle Hidden `
            -RedirectStandardError (Join-Path $work 'mysqld.err') -RedirectStandardOutput (Join-Path $work 'mysqld.out') `
            -ArgumentList '--no-defaults', "--basedir=$sourceFwd", "--datadir=$dataFwd", "--port=$port", '--bind-address=127.0.0.1',
            '--skip-grant-tables', '--skip-innodb', '--default-storage-engine=MyISAM', '--max_allowed_packet=128M',
            "--secure-file-priv=$tsvFwd"
        for ($i = 0; $i -lt 30; $i++) {
            Start-Sleep -Milliseconds 500
            if (Test-Up) { Write-Host "mysqld ready on $port"; return }
        }
        throw "mysqld did not answer; see $work\mysqld.err"
    }
    'stop' {
        Get-Process mysqld -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -eq $mysqld } | Stop-Process -Force
        Write-Host 'mysqld stopped'
    }
    'query' {
        Invoke-Sql $Sql
    }
}
