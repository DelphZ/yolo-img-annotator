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
  $msysUrl = "https://www.msys2.org/"
  Write-Host "MSYS2 is required for the GNU toolchain. The installer requires user interaction."
  $agree = Read-Host "Open MSYS2 download page in browser? (y/N)"
  if ($agree -eq 'y' -or $agree -eq 'Y') { Start-Process $msysUrl }
  Write-Host "After installing MSYS2, open MSYS2 shell and run 'pacman -Syu' then install mingw toolchain as documented on msys2.org."
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
# if ($Toolchain -eq "gnu" -or $Toolchain -eq "all") {
#   Write-Host "`nGNU toolchain requested. Checking for MSYS2..."
#   if (-not (Get-Command pacman.exe -ErrorAction SilentlyContinue)) {
#     Write-Host "MSYS2 (pacman) not found."
#     Install-MSYS2
#   } else {
#     Write-Host "MSYS2 detected."
#   }
# }

Write-Host "`nAll checks finished. Try: cargo build --release" -ForegroundColor Green
