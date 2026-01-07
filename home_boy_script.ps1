# ensure-rust-and-build.ps1

# Stop on errors
$ErrorActionPreference = "Stop"

# Check if cargo is available
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Cargo not found. Installing Rust..."

    $rustupInstaller = "rustup-init.exe"
    Invoke-WebRequest https://win.rustup.rs -OutFile $rustupInstaller
    & .\$rustupInstaller -y

    # Update PATH for current session
    $cargoBin = "$env:USERPROFILE\.cargo\bin"
    if ($env:Path -notlike "*$cargoBin*") {
        $env:Path += ";$cargoBin"
    }
}

# Build only (no run)
cargo build --release --bin solver

# Paths
$source = "target\release\solver.exe"
$destination = ".\solver.exe"

if (-not (Test-Path $source)) {
    throw "Build succeeded but binary not found at $source"
}

# Copy binary to current directory
Copy-Item $source $destination -Force

Write-Host "Binary copied to $destination"

