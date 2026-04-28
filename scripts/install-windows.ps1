# install-windows.ps1
# Crée une installation Windows de l’application Abcom et des raccourcis.

param(
    [string]$Username = "User",
    [switch]$ForceBuild
)

function Ensure-Command {
    param([string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        Write-Error "Commande '$Name' introuvable. Installe Rust avec rustup pour continuer."
        exit 1
    }
}

function New-Shortcut {
    param(
        [string]$TargetPath,
        [string]$ShortcutPath,
        [string]$Arguments = '',
        [string]$Description = 'Abcom - LAN chat'
    )

    $WshShell = New-Object -ComObject WScript.Shell
    $Shortcut = $WshShell.CreateShortcut($ShortcutPath)
    $Shortcut.TargetPath = $TargetPath
    $Shortcut.Arguments = $Arguments
    $Shortcut.WorkingDirectory = Split-Path $TargetPath
    $Shortcut.Description = $Description
    $Shortcut.IconLocation = $TargetPath
    $Shortcut.Save()
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RepoRoot = Resolve-Path "$ScriptDir\.."
Set-Location $RepoRoot

Ensure-Command -Name cargo

$TargetDir = Join-Path $RepoRoot "target\release"
$BinaryName = "abcom.exe"
$BinaryPath = Join-Path $TargetDir $BinaryName
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\abcom"
$DesktopShortcut = Join-Path ([Environment]::GetFolderPath('Desktop')) "Abcom.lnk"
$StartMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Abcom"
$StartMenuShortcut = Join-Path $StartMenuDir "Abcom.lnk"

if ($ForceBuild -or -not (Test-Path $BinaryPath)) {
    Write-Host "Compilation du binaire release..."
    cargo build --release
}

if (-not (Test-Path $BinaryPath)) {
    Write-Error "Binaire non trouvé: $BinaryPath"
    exit 1
}

Write-Host "Installation dans $InstallDir"
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
Copy-Item -Path $BinaryPath -Destination $InstallDir -Force

$ReadmeSource = Join-Path $RepoRoot "README.md"
if (Test-Path $ReadmeSource) {
    Copy-Item -Path $ReadmeSource -Destination $InstallDir -Force
}

Write-Host "Création de raccourcis"
New-Item -ItemType Directory -Path $StartMenuDir -Force | Out-Null
New-Shortcut -TargetPath (Join-Path $InstallDir $BinaryName) -ShortcutPath $DesktopShortcut -Arguments $Username
New-Shortcut -TargetPath (Join-Path $InstallDir $BinaryName) -ShortcutPath $StartMenuShortcut -Arguments $Username

Write-Host "✅ Installation Windows terminée."
Write-Host "Lance Abcom depuis le bureau ou le menu Démarrer."
Write-Host "Pour épingler à la barre des tâches, clique droit sur l’icône après lancement."