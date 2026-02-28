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
    cargo build --release --target x86_64-unknown-linux-gnu
    Copy-Item "target/x86_64-unknown-linux-gnu/release/$Bin" "$DistDir/$Bin-linux-amd64"

    # Darwin (macOS) amd64
    Write-Host "Building for darwin-amd64..." -ForegroundColor Yellow
    cargo build --release --target x86_64-apple-darwin
    Copy-Item "target/x86_64-apple-darwin/release/$Bin" "$DistDir/$Bin-darwin-amd64"

    # Windows amd64
    Write-Host "Building for windows-amd64..." -ForegroundColor Yellow
    cargo build --release --target x86_64-pc-windows-msvc
    Copy-Item "target/x86_64-pc-windows-msvc/release/$Bin.exe" "$DistDir/$Bin-windows-amd64.exe"

    Zip-All
}

function Zip-All {
    Write-Host "Compressing files..." -ForegroundColor Cyan

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
    cargo build --release

    $CargoBinPath = Join-Path $HOME ".cargo/bin"
    if (-not (Test-Path $CargoBinPath)) {
        New-Item -ItemType Directory -Path $CargoBinPath | Out-Null
    }

    Copy-Item -Path "target/release/$Bin.exe" -Destination (Join-Path $CargoBinPath "$Bin.exe") -Force
    Write-Host "Binary installed in $CargoBinPath" -ForegroundColor Green
}

switch ($Task) {
    "build-all"   { Build-All }
    "zip-all"     { Zip-All }
    "clean"       { Clean-Dist }
    "build-local" { Build-Local }
    default       { Write-Error "Task '$Task' not found. Use: build-all, zip-all, clean, build-local" }
}
