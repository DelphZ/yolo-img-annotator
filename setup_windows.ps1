<#
  setup_windows.ps1 - Windows developer environment bootstrap for Rust (MSVC or GNU)

  Usage (run from PowerShell):
    .\setup_windows.ps1 -Toolchain msvc

  Behavior:
  - If not running elevated, the script will re-launch itself with administrative privileges (UAC prompt).
  - Ensures rustup is installed (downloads rustup-init if missing).
  - Installs the requested Rust toolchain (stable-x86_64-pc-windows-msvc or stable-x86_64-pc-windows-gnu)
    and adds rustfmt/clippy components.
  - For MSVC toolchain: checks for Visual Studio Build Tools (vswhere or link.exe). If missing,
    offers to download and run the VS Build Tools installer (will prompt to confirm).
  - For GNU toolchain: offers the MSYS2 download page if pacman.exe is not found.
#>

param(
  [ValidateSet("msvc")]
  [string] $Toolchain = "all"
)

function ExitWithError($msg, $code = 1) {
  Write-Error $msg
  exit $code
}

function Ensure-Elevated {
  $isAdmin = (New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  if ($isAdmin) { return }

  Write-Host "Not running as Administrator. Attempting to relaunch script elevated (UAC prompt)..." -ForegroundColor Yellow

  $scriptPath = $MyInvocation.MyCommand.Path
  if ([string]::IsNullOrEmpty($scriptPath) -and $PSCommandPath) { $scriptPath = $PSCommandPath }
  if ([string]::IsNullOrEmpty($scriptPath)) { ExitWithError "Cannot determine script path. Please run PowerShell as Administrator and re-run the script." 2 }

  # Build argument list, preserving passed parameters
  $argList = "-NoProfile -ExecutionPolicy Bypass -File `"$scriptPath`""
  if ($PSBoundParameters.Count -gt 0) {
    $extra = $PSBoundParameters.GetEnumerator() | ForEach-Object {
      $k = $_.Key
      $v = $_.Value
      if ($v -is [System.Boolean]) {
        if ($v) { " -$k" } else { "" }
      } elseif ($null -ne $v -and $v -ne "") {
        " -$k `"$v`""
      } else { "" }
    } | Where-Object { $_ -ne "" } -join ""
    if ($extra -ne "") { $argList += $extra }
  }

  $psi = New-Object System.Diagnostics.ProcessStartInfo
  $psi.FileName = (Get-Command powershell.exe).Source
  $psi.Arguments = $argList
  $psi.Verb = "runas"    # triggers UAC
  try {
    [System.Diagnostics.Process]::Start($psi) | Out-Null
    Write-Host "Elevated process started. Exiting current process." -ForegroundColor Green
    exit 0
  } catch {
    ExitWithError "Elevation was cancelled or failed. Exiting." 3
  }
}

# Robust Start-Process wrapper: call with -ArgumentList only when we actually have args
function Run-Command($exe, $arguments) {
  # arguments can be null/empty, string, or array
  if ($null -eq $arguments) {
    Write-Host "`n> Running: $exe (no arguments)"
    $p = Start-Process -FilePath $exe -NoNewWindow -Wait -PassThru
  } elseif ($arguments -is [string]) {
    if ($arguments.Trim() -eq "") {
      Write-Host "`n> Running: $exe (no arguments)"
      $p = Start-Process -FilePath $exe -NoNewWindow -Wait -PassThru
    } else {
      Write-Host "`n> Running: $exe $arguments"
      $p = Start-Process -FilePath $exe -ArgumentList $arguments -NoNewWindow -Wait -PassThru
    }
  } elseif ($arguments -is [array]) {
    if ($arguments.Count -eq 0) {
      Write-Host "`n> Running: $exe (no arguments)"
      $p = Start-Process -FilePath $exe -NoNewWindow -Wait -PassThru
    } else {
      Write-Host "`n> Running: $exe $($arguments -join ' ')"
      $p = Start-Process -FilePath $exe -ArgumentList $arguments -NoNewWindow -Wait -PassThru
    }
  } else {
    # fallback: convert to string
    $argStr = [string]$arguments
    if ($argStr.Trim() -eq "") {
      Write-Host "`n> Running: $exe (no arguments)"
      $p = Start-Process -FilePath $exe -NoNewWindow -Wait -PassThru
    } else {
      Write-Host "`n> Running: $exe $argStr"
      $p = Start-Process -FilePath $exe -ArgumentList $argStr -NoNewWindow -Wait -PassThru
    }
  }

  return $p.ExitCode
}

function Ensure-Rustup {
  if (Get-Command rustup -ErrorAction SilentlyContinue) {
    Write-Host "rustup found: $(rustup --version)"
    return
  }

  Write-Host "rustup not found. Downloading rustup-init.exe..."
  $tmp = Join-Path $env:TEMP "rustup-init.exe"
  $url = "https://win.rustup.rs/"

  try {
    Invoke-WebRequest -Uri $url -OutFile $tmp -UseBasicParsing -ErrorAction Stop
  } catch {
    ExitWithError "Failed to download rustup-init. Check network and try again." 4
  }

  Write-Host "Running rustup installer (non-interactive -y)..."
  try {
    Start-Process -FilePath $tmp -ArgumentList "-y" -Wait -NoNewWindow
    Remove-Item $tmp -ErrorAction SilentlyContinue
  } catch {
    ExitWithError "Failed to run rustup installer." 5
  }
}

function Ensure-Toolchain {
  param($tool)
  if ($tool -eq "msvc") { $triplet = "stable-x86_64-pc-windows-msvc" } else { $triplet = "stable-x86_64-pc-windows-gnu" }

  Write-Host "Installing Rust toolchain: $triplet (if missing)..."
  try {
    & rustup toolchain install $triplet
    & rustup default $triplet
    & rustup component add rustfmt --toolchain $triplet
    & rustup component add clippy --toolchain $triplet
  } catch {
    ExitWithError "Failed to install or configure Rust toolchain: $_" 6
  }
}

function Has-MSVC {
  $vswherePath = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
  if (Test-Path $vswherePath) {
    try {
      $res = & $vswherePath -latest -products * -requires Microsoft.Component.MSBuild -property installationPath 2>$null
      if ($res) { return $true }
    } catch {}
  }
  if (Get-Command link.exe -ErrorAction SilentlyContinue) { return $true }
  return $false
}

function Install-VSBuildTools {
  $vsURL = "https://aka.ms/vs/17/release/vs_BuildTools.exe"
  Write-Host "The script can download Visual Studio Build Tools and install the C++ workload (VCTools)." -ForegroundColor Yellow
  $agree = Read-Host "Install Visual Studio Build Tools now? This requires admin rights and may take a long time. (y/N)"
  if ($agree -ne 'y' -and $agree -ne 'Y') { Write-Host "Skipping VS Build Tools installation."; return }

  $tmp = Join-Path $env:TEMP "vs_buildtools.exe"
  Write-Host "Downloading VS Build Tools bootstrapper to $tmp ..."
  try {
    Invoke-WebRequest -Uri $vsURL -OutFile $tmp -UseBasicParsing -ErrorAction Stop
  } catch {
    ExitWithError "Failed to download VS Build Tools installer." 7
  }

  $args = "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools"
  Write-Host "Starting silent install of Visual Studio Build Tools (this may take many minutes)..."
  $rc = Run-Command $tmp $args
  if ($rc -ne 0) { ExitWithError "VS Build Tools installer returned non-zero exit code $rc" 8 }
  Write-Host "VS Build Tools install completed. You may need to restart or wait for environment variables to refresh."
}

function Install-MSYS2 {
  # Silent / automatic MSYS2 installer + pacman setup + add mingw64 bin to User PATH
  $msysBin = 'C:\msys64\ucrt64\bin'
  if (Get-Command pacman.exe -ErrorAction SilentlyContinue) {
    Write-Host "MSYS2/pacman already present. Skipping installer."
    return
  }

  Write-Host "MSYS2 not found. Attempting automatic silent installation..." -ForegroundColor Yellow

  # Prefer winget if available (more robust)
  if (Get-Command winget.exe -ErrorAction SilentlyContinue) {
    Write-Host "winget detected. Installing MSYS2 via winget (silent)..."
    $wgArgs = "install --id MSYS2.MSYS2 -e --accept-source-agreements --accept-package-agreements"
    $rc = Run-Command (Get-Command winget.exe).Source $wgArgs
    if ($rc -ne 0) { Write-Host "winget install failed (code $rc). Falling back to direct installer." }
    else { Write-Host "winget install completed." }
  }

  # If pacman still not found, fall back to direct download + silent installer
  if (-not (Get-Command pacman.exe -ErrorAction SilentlyContinue)) {
    # GitHub redirect for the latest MSYS2 installer
    $installerUrl = "https://github.com/msys2/msys2-installer/releases/latest/download/msys2-x86_64-latest.exe"
    $tmpInstaller = Join-Path $env:TEMP "msys2-installer.exe"
    Write-Host "Downloading MSYS2 installer to $tmpInstaller ..."
    try {
      Invoke-WebRequest -Uri $installerUrl -OutFile $tmpInstaller -UseBasicParsing -ErrorAction Stop
    } catch {
      ExitWithError "Failed to download MSYS2 installer from $installerUrl. Please install MSYS2 manually." 20
    }

    # Attempt silent install. MSYS2 installer is NSIS-based and supports /S for silent.
    Write-Host "Running MSYS2 installer silently (this may take a few minutes)..."
    $rc = Run-Command $tmpInstaller "/S"
    if ($rc -ne 0) {
      Write-Host "Silent installer returned code $rc. Trying interactive run (for diagnostics)..."
      # Try interactive as fallback to let user see errors
      try {
        Start-Process -FilePath $tmpInstaller -ArgumentList "" -Wait
      } catch {
        ExitWithError "MSYS2 installer failed. Please install MSYS2 manually from https://www.msys2.org/." 21
      }
    }

    # Clean up downloaded installer (best-effort)
    try { Remove-Item $tmpInstaller -ErrorAction SilentlyContinue } catch {}
  }

  # Wait a short time for MSYS2 to settle, then run pacman updates and install mingw toolchain
  $bashPath = "C:\msys64\usr\bin\bash.exe"
  if (-not (Test-Path $bashPath)) {
    # try alternative path if installed to Program Files or similar (rare)
    $bashPath = (Get-Command bash.exe -ErrorAction SilentlyContinue | Select-Object -First 1).Source
  }
  if (-not $bashPath -or -not (Test-Path $bashPath)) {
    ExitWithError "Unable to find MSYS2 bash.exe after installation. Please check MSYS2 install location." 22
  }

  Write-Host "Running MSYS2 pacman updates and installing mingw-w64-x86_64-toolchain (non-interactive)..."
  # Use -lc to run commands from Windows; combine updates then install toolchain
  $cmd = "pacman -Syu --noconfirm; pacman -Su --noconfirm; pacman -S --noconfirm --needed mingw-w64-x86_64-toolchain mingw-w64-x86_64-binutils"
  $rc = Run-Command $bashPath "-lc `"$cmd`""
  if ($rc -ne 0) {
    Write-Host "pacman returned non-zero exit code $rc. You may need to run MSYS2 shell manually and run: pacman -Syu ; pacman -S mingw-w64-x86_64-toolchain" -ForegroundColor Yellow
  } else {
    Write-Host "MSYS2 packages installed."
  }

  # Add mingw64 bin to User PATH (so dlltool.exe and other tools are visible to PowerShell/cargo)
  $current = [Environment]::GetEnvironmentVariable('PATH', 'User') -or ""
  if ($current -notlike "*$msysBin*") {
    $new = if ($current -eq "") { $msysBin } else { "$current;$msysBin" }
    try {
      [Environment]::SetEnvironmentVariable('PATH', $new, 'User')
      Write-Host "Added $msysBin to user PATH. Please restart your terminal for changes to take effect."
    } catch {
      Write-Host "Failed to modify User PATH automatically. Please add $msysBin to your PATH manually." -ForegroundColor Yellow
    }
  } else {
    Write-Host "$msysBin already present in User PATH."
  }

  Write-Host "MSYS2 installation (attempt) finished. Verify by running: Get-Command dlltool.exe"
}


# ---------------------- main ----------------------
Ensure-Elevated

Write-Host "`n=== Rust Windows Setup Script ===`n"

Ensure-Rustup
Ensure-Toolchain -tool $Toolchain

if ($Toolchain -eq "msvc" -or $Toolchain -eq "all") {
  if (-not (Has-MSVC)) {
    Write-Host "`nMSVC toolchain (Visual C++ build tools) not detected."
    Install-VSBuildTools
  } else {
    Write-Host "MSVC toolchain detected."
  }
}
if ($Toolchain -eq "gnu" -or $Toolchain -eq "all") {
  Write-Host "`nGNU toolchain requested. Checking for MSYS2..."
  if (-not (Get-Command pacman.exe -ErrorAction SilentlyContinue)) {
    Write-Host "MSYS2 (pacman) not found."
    Install-MSYS2
  } else {
    Write-Host "MSYS2 detected."
  }
}

Write-Host "`nAll checks finished. Try: cargo build --release" -ForegroundColor Green
