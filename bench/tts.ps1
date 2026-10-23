param([string]$Voice, [string]$Out, [string]$Text)
Add-Type -AssemblyName System.Speech
$s = New-Object System.Speech.Synthesis.SpeechSynthesizer
try { $s.SelectVoice("Microsoft $Voice") } catch { $s.SelectVoice("Microsoft Hortense Desktop") }
$s.SetOutputToWaveFile($Out)
$s.Speak($Text)
$s.Dispose()
