# Generates catalog/models.json from the built-in manifest in
# src-tauri/src/services/models/catalog.rs — the single source of truth.
# The parity test in CI proves the JSON and the compiled list stay identical,
# and the catalog-pins job re-verifies every sha256/size against the
# Hugging Face API.
#
# Usage: powershell -NoProfile -File scripts/gen-catalog.ps1

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$srcPath = Join-Path $repo "src-tauri/src/services/models/catalog.rs"
$outPath = Join-Path $repo "catalog/models.json"

$src = [System.IO.File]::ReadAllText($srcPath)

$start = $src.IndexOf("let models = vec![")
if ($start -lt 0) { throw "could not find the models vec in catalog.rs" }
$end = $src.IndexOf("];", $start)
$body = $src.Substring($start, $end - $start)

$entryRegex = [regex]'entry\(\s*"([^"]*)",\s*"([^"]*)",\s*"([^"]*)",\s*"([^"]*)",\s*"([^"]*)",\s*([\d_]+),\s*([\d.]+),\s*"([^"]*)",\s*"([0-9a-f]{64})",\s*"([^"]*)",\s*"([^"]*)",\s*(vec!\[[\s\S]*?\]|vec!\[\])\s*,\s*\)\s*(?:\.with_context\(([\d_]+)\)\s*)?(?:\.with_dest\("([^"]*)"\)\s*)?'
$companionRegex = [regex]'companion\(\s*"([^"]+)",\s*"([0-9a-f]{64})",\s*([\d_]+),\s*"([^"]+)",?\s*\)'

$models = New-Object System.Collections.Generic.List[object]
foreach ($m in $entryRegex.Matches($body)) {
    $companions = New-Object System.Collections.Generic.List[object]
    foreach ($c in $companionRegex.Matches($m.Groups[12].Value)) {
        $companions.Add([ordered]@{
            url       = $c.Groups[1].Value
            sha256    = $c.Groups[2].Value
            file_size = [int64]($c.Groups[3].Value.Replace("_", ""))
            dest      = $c.Groups[4].Value
        })
    }
    $e = [ordered]@{
        id              = $m.Groups[1].Value
        name            = $m.Groups[2].Value
        params          = $m.Groups[3].Value
        kind            = $m.Groups[4].Value
        license         = $m.Groups[5].Value
        file_size       = [int64]($m.Groups[6].Value.Replace("_", ""))
        ram_estimate_gb = [double]::Parse($m.Groups[7].Value, [System.Globalization.CultureInfo]::InvariantCulture)
        url             = $m.Groups[8].Value
        sha256          = $m.Groups[9].Value
        quant           = $m.Groups[10].Value
        description     = $m.Groups[11].Value
    }
    if ($m.Groups[13].Success) { $e["context_tokens"] = [int64]$m.Groups[13].Value.Replace("_", "") }
    if ($m.Groups[14].Success) { $e["dest"] = $m.Groups[14].Value }
    $e["companions"] = $companions
    $models.Add($e)
}

# Digest mirrors CatalogManifest::compute_signature: a sequential SHA-256
# over id, sha256, and the little-endian u64 size of every entry, then the
# same three fields of every companion.
$hasher = [System.Security.Cryptography.SHA256]::Create()
$stream = New-Object System.Collections.Generic.List[byte]
foreach ($m in $models) {
    $stream.AddRange([System.Text.Encoding]::UTF8.GetBytes($m["id"]))
    $stream.AddRange([System.Text.Encoding]::UTF8.GetBytes($m["sha256"]))
    $stream.AddRange([System.BitConverter]::GetBytes([uint64]$m["file_size"]))
    foreach ($c in $m["companions"]) {
        $stream.AddRange([System.Text.Encoding]::UTF8.GetBytes($c["sha256"]))
        $stream.AddRange([System.BitConverter]::GetBytes([uint64]$c["file_size"]))
    }
}
$bytes = $stream.ToArray()
$signature = [System.BitConverter]::ToString($hasher.ComputeHash($bytes)).Replace("-", "").ToLowerInvariant()

# Version and updated_at come from the source manifest so the two stay in
# lock-step (the parity test compares the full manifest).
$version = [regex]::Match($src, 'version:\s*([\d_]+),').Groups[1].Value.Replace("_", "")
$updatedAt = [regex]::Match($src, 'updated_at:\s*([\d_]+),').Groups[1].Value.Replace("_", "")

$manifest = [ordered]@{
    version    = [int64]$version
    updated_at = [int64]$updatedAt
    signature  = $signature
    models     = $models
}

$json = $manifest | ConvertTo-Json -Depth 6
[System.IO.File]::WriteAllText($outPath, $json, (New-Object System.Text.UTF8Encoding($false)))
Write-Output ("wrote $outPath with " + $models.Count + " models, signature " + $signature)
