# Get the absolute path to the dlltool folder
$DllToolDir = Resolve-Path ".\dlltool"

Write-Host "Adding $DllToolDir to user PATH..."

# Get current user PATH
$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")

# Check if the directory is already in PATH
if ($currentPath -notmatch [Regex]::Escape($DllToolDir)) {
    # Append the directory to PATH
    $newPath = "$currentPath;$DllToolDir"
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "✅ Successfully added to PATH."
    Write-Host "Please restart PowerShell or log out/in for changes to take effect."
} else {
    Write-Host "ℹ️ The directory is already in your PATH."
}
