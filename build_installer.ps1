# QuillScribe Installer Build Script (PowerShell)
# This script builds the executable and creates the NSIS installer

Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "  QuillScribe Installer Build Script" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan
Write-Host ""

# Function to check if a command exists
function Test-CommandExists {
    param($Command)
    try {
        Get-Command $Command -ErrorAction Stop
        return $true
    }
    catch {
        return $false
    }
}

# Check if Python is available
if (-not (Test-CommandExists "python")) {
    Write-Host "ERROR: Python is not installed or not in PATH" -ForegroundColor Red
    Read-Host "Press Enter to exit"
    exit 1
}

Write-Host "Python version:" -ForegroundColor Green
python --version

# Check if PyInstaller is installed
try {
    python -c "import PyInstaller" 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "PyInstaller not found"
    }
}
catch {
    Write-Host "Installing PyInstaller and Pillow..." -ForegroundColor Yellow
    pip install pyinstaller Pillow
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: Failed to install PyInstaller" -ForegroundColor Red
        Read-Host "Press Enter to exit"
        exit 1
    }
}

# Clean previous builds
Write-Host "Cleaning previous builds..." -ForegroundColor Yellow
if (Test-Path "build") {
    Remove-Item -Recurse -Force "build"
}
if (Test-Path "dist") {
    Remove-Item -Recurse -Force "dist"
}
if (Test-Path "QuillScribe.spec") {
    Remove-Item -Force "QuillScribe.spec"
}
if (Test-Path "QuillScribe-Installer.exe") {
    Remove-Item -Force "QuillScribe-Installer.exe"
}

# Install dependencies
Write-Host "Installing dependencies..." -ForegroundColor Yellow
pip install -r requirements.txt
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Failed to install dependencies" -ForegroundColor Red
    Read-Host "Press Enter to exit"
    exit 1
}

# Build the executable
Write-Host "Building executable with PyInstaller using custom spec..." -ForegroundColor Yellow
python -m PyInstaller QuillScribe_Custom.spec --clean
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Failed to build executable" -ForegroundColor Red
    Read-Host "Press Enter to exit"
    exit 1
}

# Function to find NSIS makensis executable
function Get-MakensisPath {
    # First check if makensis is in PATH
    if (Test-CommandExists "makensis") {
        return "makensis"
    }
    
    # Check common NSIS installation directories
    $nsisLocations = @(
        "C:\Program Files (x86)\NSIS\makensis.exe",
        "C:\Program Files\NSIS\makensis.exe",
        "${env:ProgramFiles(x86)}\NSIS\makensis.exe",
        "$env:ProgramFiles\NSIS\makensis.exe"
    )
    
    foreach ($location in $nsisLocations) {
        if (Test-Path $location) {
            Write-Host "Found NSIS at: $location" -ForegroundColor Green
            return $location
        }
    }
    
    return $null
}

# Check if NSIS is available
$makensisPath = Get-MakensisPath
if ($null -eq $makensisPath) {
    Write-Host ""
    Write-Host "ERROR: NSIS (makensis) not found" -ForegroundColor Red
    Write-Host "Please install NSIS from https://nsis.sourceforge.io/" -ForegroundColor Yellow
    Write-Host "Checked the following locations:" -ForegroundColor Yellow
    Write-Host "  - System PATH" -ForegroundColor Yellow
    Write-Host "  - C:\Program Files (x86)\NSIS\" -ForegroundColor Yellow
    Write-Host "  - C:\Program Files\NSIS\" -ForegroundColor Yellow
    Write-Host ""
    Read-Host "Press Enter to exit"
    exit 1
}

# Create the installer
Write-Host "Creating NSIS installer..." -ForegroundColor Yellow
& $makensisPath installer.nsi
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Failed to create installer" -ForegroundColor Red
    Read-Host "Press Enter to exit"
    exit 1
}

Write-Host ""
Write-Host "=========================================" -ForegroundColor Green
Write-Host "  BUILD COMPLETED SUCCESSFULLY!" -ForegroundColor Green
Write-Host "=========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Executable location: dist\QuillScribe\QuillScribe.exe" -ForegroundColor Cyan
Write-Host "Installer location: QuillScribe-Installer.exe" -ForegroundColor Cyan
Write-Host ""

# Option to run the installer
$runInstaller = Read-Host "Do you want to run the installer now? (y/n)"
if ($runInstaller -eq "y" -or $runInstaller -eq "Y") {
    Start-Process ".\QuillScribe-Installer.exe"
}

Read-Host "Press Enter to exit"
