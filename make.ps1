param (
    [Parameter(Position=0)]
    [String]$Task = "build-all"
)

$DistDir = "dist"
$Bin = "gref"

function Clean-Dist {
    if (Test-Path $DistDir) {
        Remove-Item -Recurse -Force $DistDir
    }
    New-Item -ItemType Directory -Path $DistDir | Out-Null
}

function Build-All {
    Clean-Dist

    # Linux amd64
    Write-Host "Building for linux-amd64..." -ForegroundColor Yellow
    $env:GOOS="linux"; $env:GOARCH="amd64"
    go build -ldflags="-s -w" -o "$DistDir/$Bin-linux-amd64"

    # Darwin (macOS) amd64
    Write-Host "Building for darwin-amd64..." -ForegroundColor Yellow
    $env:GOOS="darwin"; $env:GOARCH="amd64"
    go build -ldflags="-s -w" -o "$DistDir/$Bin-darwin-amd64"

    # Windows amd64
    Write-Host "Building for windows-amd64..." -ForegroundColor Yellow
    $env:GOOS="windows"; $env:GOARCH="amd64"
    go build -ldflags="-s -w" -o "$DistDir/$Bin-windows-amd64.exe"

    $env:GOOS=""; $env:GOARCH=""

    Zip-All
}

function Zip-All {
    Write-Host "Compressione dei file in corso..." -ForegroundColor Cyan
    
    $targets = @(
        @{ File = "$Bin-linux-amd64"; Zip = "$Bin-linux-amd64.zip" },
        @{ File = "$Bin-darwin-amd64"; Zip = "$Bin-darwin-amd64.zip" },
        @{ File = "$Bin-windows-amd64.exe"; Zip = "$Bin-windows-amd64.zip" }
    )

    foreach ($target in $targets) {
        $filePath = Join-Path $DistDir $target.File
        $zipPath = Join-Path $DistDir $target.Zip
        
        if (Test-Path $filePath) {
            Write-Host "Zipping $($target.Zip)..."
            Compress-Archive -Path $filePath -DestinationPath $zipPath -Update
        }
    }
}

function Build-Local {
    Write-Host "Building local..." -ForegroundColor Green
    go build -ldflags="-s -w" -o "$Bin.exe"
    
    $GoBinPath = Join-Path $HOME "go/bin"
    if (-not (Test-Path $GoBinPath)) {
        New-Item -ItemType Directory -Path $GoBinPath | Out-Null
    }
    
    Move-Item -Path "$Bin.exe" -Destination (Join-Path $GoBinPath "$Bin.exe") -Force
    Write-Host "Binary installed in $GoBinPath" -ForegroundColor Green
}

switch ($Task) {
    "build-all"  { Build-All }
    "zip-all"    { Zip-All }
    "clean"      { Clean-Dist }
    "build-local" { Build-Local }
    default      { Write-Error "Task '$Task' not found. Use: build-all, zip-all, clean, build-local" }
}