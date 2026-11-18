$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$PawnioDir = Join-Path $RepoRoot "pawnio"
$Manifest = Join-Path $PawnioDir "modules.sha256"

$Expected = @{}
foreach ($line in Get-Content $Manifest) {
    if ($line -match '^\s*([0-9a-fA-F]{64})\s+(\S+)\s*$') {
        $Expected[$matches[2]] = $matches[1].ToLowerInvariant()
    }
}

$AllOk = $true
foreach ($name in $Expected.Keys) {
    $p = Join-Path $PawnioDir $name
    if (-not (Test-Path $p)) { $AllOk = $false; break }
    $actual = (Get-FileHash $p -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $Expected[$name]) { $AllOk = $false; break }
}

if ($AllOk) {
    Write-Output "PawnIO modules already present and verified against modules.sha256."
    exit 0
}

$TmpDir = Join-Path $env:TEMP ("pawnio-modules-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $TmpDir | Out-Null
try {
    $ZipPath = Join-Path $TmpDir "release_0_2_12.zip"
    $Url = "https://github.com/namazso/PawnIO.Modules/releases/download/0.2.12/release_0_2_12.zip"
    Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing
    $ExtractDir = Join-Path $TmpDir "extracted"
    Expand-Archive -Path $ZipPath -DestinationPath $ExtractDir -Force

    foreach ($name in $Expected.Keys) {
        $found = Get-ChildItem -Path $ExtractDir -Filter $name -Recurse | Select-Object -First 1
        if (-not $found) {
            throw "Missing expected module '$name' in PawnIO.Modules 0.2.12 archive"
        }
        $actual = (Get-FileHash $found.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -ne $Expected[$name]) {
            throw "SHA-256 mismatch for '$name': expected $($Expected[$name]), got $actual"
        }
        Copy-Item -Path $found.FullName -Destination (Join-Path $PawnioDir $name) -Force
    }
    Write-Output "Verified and staged PawnIO 0.2.12 modules into $PawnioDir"
} finally {
    Remove-Item -Path $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
}
