# Windows system voices (SAPI5) through System.Speech. Called by sapi_engine.py.
#   -Action list              prints the installed voices as JSON
#   -Action speak -JobFile <file> speaks the JSON job file into a WAV file
# Job fields: voice, rate (-10..10), text, output, ssml (optional, used for pitch).
param(
    [Parameter(Mandatory = $true)][string]$Action,
    [string]$JobFile
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Speech
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
try {
    if ($Action -eq 'list') {
        $voices = @($synth.GetInstalledVoices() | Where-Object { $_.Enabled } | ForEach-Object {
            $info = $_.VoiceInfo
            [pscustomobject]@{
                name    = $info.Name
                culture = $info.Culture.Name
                gender  = $info.Gender.ToString()
                age     = $info.Age.ToString()
            }
        })
        ConvertTo-Json -InputObject $voices -Compress
    }
    elseif ($Action -eq 'speak') {
        # PowerShell variable names are case-insensitive: never name this variable like a parameter ($job vs $Job).
        $request = Get-Content -LiteralPath $JobFile -Raw -Encoding UTF8 | ConvertFrom-Json
        $synth.SelectVoice([string]$request.voice)
        $synth.Rate = [int]$request.rate
        $synth.SetOutputToWaveFile([string]$request.output)
        $pitchApplied = $false
        if ($request.ssml) {
            try {
                $synth.SpeakSsml([string]$request.ssml)
                $pitchApplied = $true
            }
            catch {
                # This voice rejected the pitch markup: start over without it.
                $synth.SetOutputToNull()
                $synth.SetOutputToWaveFile([string]$request.output)
                $synth.Speak([string]$request.text)
            }
        }
        else {
            $synth.Speak([string]$request.text)
        }
        ConvertTo-Json -InputObject @{ pitchApplied = $pitchApplied } -Compress
    }
    else {
        throw "Unknown action: $Action"
    }
}
finally {
    $synth.Dispose()
}
