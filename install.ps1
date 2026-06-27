param(
    [string]$Version = "latest",
    [string]$Prefix = "",
    [string]$BinDir = "",
    [string]$VimDir = "",
    [switch]$NoVim,
    [switch]$VimPack,
    [string]$Repo = $env:GREF_REPO
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Repo)) {
    $Repo = "albertize/gref"
}

function Fail($Message) {
    Write-Error "gref install: $Message"
    exit 1
}

function Get-AssetName {
    switch ($env:PROCESSOR_ARCHITECTURE) {
        "AMD64" { $arch = "amd64" }
        "ARM64" { $arch = "arm64" }
        default { Fail "unsupported architecture: $env:PROCESSOR_ARCHITECTURE" }
    }
    "gref-windows-$arch.zip"
}

function Get-BinDir {
    if ($BinDir) {
        return $BinDir.TrimEnd([char]'\')
    }
    if ($Prefix) {
        return (Join-Path $Prefix "bin")
    }
    Join-Path $HOME "bin"
}

function Get-VimDir {
    if ($VimDir) {
        return $VimDir.TrimEnd([char]'\')
    }
    if ($Prefix) {
        return (Join-Path $Prefix "vimfiles")
    }
    if ($VimPack) {
        return (Join-Path $HOME "vimfiles\pack\gref\start\gref")
    }
    Join-Path $HOME "vimfiles"
}

function Install-File($Source, $Destination) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Destination) | Out-Null
    Copy-Item -Force $Source $Destination
}

function Install-Package($PackageDir) {
    $bin = Join-Path $PackageDir "bin\gref.exe"
    if (!(Test-Path $bin)) {
        Fail "package is missing bin\gref.exe"
    }

    $targetBinDir = Get-BinDir
    $targetBin = Join-Path $targetBinDir "gref.exe"
    Install-File $bin $targetBin
    Write-Host "installed gref to $targetBin"

    if (!$NoVim) {
        $plugin = Join-Path $PackageDir "vim\plugin\gref.vim"
        $autoload = Join-Path $PackageDir "vim\autoload\gref.vim"
        if (!(Test-Path $plugin)) {
            Fail "package is missing vim\plugin\gref.vim"
        }
        if (!(Test-Path $autoload)) {
            Fail "package is missing vim\autoload\gref.vim"
        }

        $targetVimDir = Get-VimDir
        Install-File $plugin (Join-Path $targetVimDir "plugin\gref.vim")
        Install-File $autoload (Join-Path $targetVimDir "autoload\gref.vim")
        Write-Host "installed Vim runtime to $targetVimDir"
    }

    $pathEntries = ($env:PATH -split ";") | ForEach-Object { $_.TrimEnd([char]'\') }
    if ($pathEntries -notcontains $targetBinDir.TrimEnd([char]'\')) {
        Write-Host "note: $targetBinDir is not in PATH"
    }
}

$scriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { (Get-Location).Path }
if (Test-Path (Join-Path $scriptDir "bin\gref.exe")) {
    Install-Package $scriptDir
    exit 0
}

$asset = Get-AssetName
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("gref-install-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
    if ($Version -eq "latest") {
        $baseUrl = "https://github.com/$Repo/releases/latest/download"
    } else {
        $baseUrl = "https://github.com/$Repo/releases/download/$Version"
    }

    $assetPath = Join-Path $tmp $asset
    $sumsPath = Join-Path $tmp "SHA256SUMS"

    Write-Host "downloading $asset from $Repo ($Version)"
    Invoke-WebRequest -UseBasicParsing "$baseUrl/$asset" -OutFile $assetPath
    Invoke-WebRequest -UseBasicParsing "$baseUrl/SHA256SUMS" -OutFile $sumsPath

    $assetPattern = '(\s|\*)' + [regex]::Escape($asset) + '$'
    $checksumLine = Get-Content $sumsPath | Where-Object { $_ -match $assetPattern } | Select-Object -First 1
    if (!$checksumLine) {
        Fail "checksum for $asset not found"
    }
    $expected = ($checksumLine -split "\s+")[0].ToUpperInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 $assetPath).Hash.ToUpperInvariant()
    if ($actual -ne $expected) {
        Fail "checksum mismatch for $asset"
    }

    $extractDir = Join-Path $tmp "package"
    Expand-Archive -Path $assetPath -DestinationPath $extractDir
    $packageRoot = Get-ChildItem -Path $extractDir -Directory | Select-Object -First 1
    if (!$packageRoot) {
        Fail "archive did not contain a package directory"
    }

    Install-Package $packageRoot.FullName
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
