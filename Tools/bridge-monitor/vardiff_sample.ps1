param(
    [int]$Minutes = 10,
    [int]$TailLines = 200
)

$ErrorActionPreference = 'Stop'

$log = Get-ChildItem -Path . -Filter 'rustbridge_*.log' | Sort-Object LastWriteTime | Select-Object -Last 1
if (-not $log) {
    Write-Host 'No rustbridge_*.log files found in the current directory. Make sure RustBridge is running with log-to-file enabled.' -ForegroundColor Red
    exit 1
}

$sample = 'vardiff_samples.log'

Remove-Item $sample -ErrorAction SilentlyContinue

"=== FULL LOG FROM START ===" | Out-File $sample -Encoding UTF8
Get-Content $log.FullName | Out-File $sample -Append -Encoding UTF8

for ($i = 1; $i -le $Minutes; $i++) {
    "`n=== SAMPLE $i $(Get-Date -Format o) ===" | Out-File $sample -Append -Encoding UTF8
    Get-Content $log.FullName -Tail $TailLines | Out-File $sample -Append -Encoding UTF8
    Start-Sleep -Seconds 60
}

Write-Host "Sampling complete. Results written to $sample" -ForegroundColor Green
