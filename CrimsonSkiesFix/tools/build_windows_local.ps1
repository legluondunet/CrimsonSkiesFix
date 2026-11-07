# Build and validate a CrimsonSkiesFix Windows x86 candidate without GitHub Actions.
# Run from a Windows MSVC developer environment with Rust, Python 3 and Git.
# Does not access game files or overwrite the user's existing game DLL / INI.
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-Exit([string]$Description) {
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE"
    }
}

function Run-Python([string[]]$Arguments) {
    if (Get-Command python -ErrorAction SilentlyContinue) {
        & python @Arguments
    } elseif (Get-Command py -ErrorAction SilentlyContinue) {
        & py -3 @Arguments
    } else {
        throw 'Python 3 not found. Install Python 3, then retry.'
    }
    Assert-Exit 'Python verification'
}

if ($env:OS -ne 'Windows_NT') {
    throw 'Use a native Windows x86/x64 machine for the MSVC build and loader smoke test.'
}
foreach ($command in @('cargo', 'rustup', 'git')) {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
        throw "Missing '$command'. Install Rust stable (rustup), Git for Windows, and the Visual Studio C++ build tools."
    }
}

$crateRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$repoRoot = (Resolve-Path (Join-Path $crateRoot '..')).Path
$target = 'i686-pc-windows-msvc'
$oldRustFlags = $env:RUSTFLAGS

Push-Location $repoRoot
try {
    $commit = (& git rev-parse HEAD).Trim()
    Assert-Exit 'git rev-parse HEAD'
    if ($commit -cnotmatch '^[0-9a-fA-F]{40}$') {
        throw "Invalid source revision: $commit"
    }
    $dirty = & git status --porcelain --untracked-files=no
    Assert-Exit 'git status'
    if ($dirty) {
        throw 'Tracked files have local changes. Commit/stash them before building a provenance-bound candidate.'
    }
} finally {
    Pop-Location
}

$commit = $commit.ToLowerInvariant()
$short = $commit.Substring(0, 8)
$output = Join-Path (Split-Path $repoRoot -Parent) "CrimsonSkiesFix-local-$short"
$archive = "$output.zip"
if ((Test-Path -LiteralPath $output) -or (Test-Path -LiteralPath $archive)) {
    throw "Candidate path already exists: $output or $archive. Move it before rebuilding."
}

Write-Host "LOCAL-CI: source=$commit target=$target"
Write-Host 'LOCAL-CI: first verify the C++ x86 MSVC linker and Windows SDK are installed.'
Write-Host 'LOCAL-CI: run from an x86 Native Tools or x64_x86 Cross Tools developer PowerShell if the linker is missing.'

Push-Location $crateRoot
try {
    # Match the GitHub Windows workflow's CRT linkage setting.
    $env:RUSTFLAGS = '-C target-feature=+crt-static'

    & rustup target add $target
    Assert-Exit 'rustup target add'

    & cargo test --locked --target $target
    Assert-Exit 'Rust i686 unit tests'

    & cargo build --locked --release --target $target
    Assert-Exit 'Release x86 DirectDraw proxy build'

    & cargo build --locked --release --target $target --bin ddraw_loader_smoke
    Assert-Exit 'Native loader smoke build'

    $dll = Join-Path $crateRoot "target\$target\release\ddraw.dll"
    $smoke = Join-Path $crateRoot "target\$target\release\ddraw_loader_smoke.exe"
    if (-not (Test-Path -LiteralPath $dll) -or -not (Test-Path -LiteralPath $smoke)) {
        throw 'The Windows i686 build did not produce both the DLL and loader smoke executable.'
    }

    # This checks all 22 names and canonical ordinals in the actual PE32 DLL.
    Run-Python @('tools/verify_ddraw_exports.py', '--dll', $dll,
                 '--definitions', 'src/ddraw_exports.def')

    & $smoke $dll
    Assert-Exit 'Native Windows x86 DLL loader and GetProcAddress smoke'

    # Exactly the five-file layout used by the GitHub release candidate workflow.
    $nested = Join-Path $output 'CrimsonSkiesFix'
    New-Item -ItemType Directory -Path $nested -Force | Out-Null
    Copy-Item -LiteralPath $dll -Destination (Join-Path $output 'ddraw.dll')
    Copy-Item -LiteralPath (Join-Path $crateRoot 'CrimsonSkiesFix\CrimsonSkiesFix.ini.example') -Destination (Join-Path $nested 'CrimsonSkiesFix.ini.example')
    Copy-Item -LiteralPath (Join-Path $crateRoot 'README.md') -Destination (Join-Path $nested 'README.md')
    Copy-Item -LiteralPath (Join-Path $repoRoot 'LICENSE') -Destination (Join-Path $nested 'LICENSE')

    $cargoToml = Get-Content -LiteralPath (Join-Path $crateRoot 'Cargo.toml') -Raw
    $versionMatch = [regex]::Match($cargoToml, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $versionMatch.Success) { throw 'Cannot read the Cargo package version.' }
    $version = $versionMatch.Groups[1].Value
    $hash = (Get-FileHash -LiteralPath (Join-Path $output 'ddraw.dll') -Algorithm SHA256).Hash.ToLowerInvariant()
    $size = (Get-Item -LiteralPath (Join-Path $output 'ddraw.dll')).Length
    $manifest = @(
        'Project=CrimsonSkiesFix'
        "Version=$version"
        "Commit=$commit"
        "LogIdentity=CrimsonSkiesFix $version-$short x86"
        'Architecture=Windows-x86'
        "DllSha256=$hash"
        "DllSizeBytes=$size"
        "Source=https://github.com/legluondunet/CrimsonSkiesFix/tree/$commit"
    )
    $manifest | Set-Content -LiteralPath (Join-Path $nested 'build-info.txt') -Encoding utf8

    Run-Python @('tools/verify_release_candidate.py', '--directory', $output,
                 '--expected-commit', $commit, '--cargo-toml',
                 (Join-Path $crateRoot 'Cargo.toml'), '--license',
                 (Join-Path $repoRoot 'LICENSE'))

    # A local test build is NOT a signed/released artifact.
    Compress-Archive -Path (Join-Path $output '*') -DestinationPath $archive
    Write-Host ''
    Write-Host 'LOCAL-CI: PASS — Rust tests, complete x86 DDRAW exports, loader smoke, 5-file manifest.'
    Write-Host "LOCAL-CI: archive=$archive"
    Write-Host "LOCAL-CI: commit=$commit"
    Write-Host "LOCAL-CI: dll_sha256=$hash"
    Write-Host 'LOCAL-CI: NOT a Windows gameplay acceptance test, nor an official release.'
} finally {
    $env:RUSTFLAGS = $oldRustFlags
    Pop-Location
}
