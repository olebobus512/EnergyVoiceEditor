"""Debug engine: a stdlib-only stand-in for a real TTS model.

It turns each character into a short tone so the whole pipeline (IPC, WAV
writing, playback, saving) can be verified before a real model is integrated.
The output is NOT speech.
"""

import array
import math
import sys
import wave

SAMPLE_RATE = 22050
MAX_TEXT_LENGTH = 2000

VOICES = {
    "debug-a": {"id": "debug-a", "name": "Debug Tone A", "languages": ["any"], "base_pitch": 180.0},
    "debug-b": {"id": "debug-b", "name": "Debug Tone B", "languages": ["any"], "base_pitch": 240.0},
}
PUBLIC_KEYS = ("id", "name", "languages")

TONE_SEC = 0.09
SPACE_SEC = 0.12
PAUSE_SEC = 0.25
PAUSE_CHARS = ".,!?;:"


class DebugEngine:
    name = "debug"
    label = "Debug tones (not speech)"
    # Only offered when no real speech engine has any voice.
    fallback = True

    def configure(self, voices_dir):
        """Nothing to configure."""

    def unavailable_reason(self):
        return None

    def list_voices(self, refresh=False):
        return [
            {**{key: voice[key] for key in PUBLIC_KEYS}, "engine": self.name, "gender": "", "supportsPitch": True}
            for voice in VOICES.values()
        ]

    def synthesize(self, text, voice_id, output_path, speed, pitch, progress):
        if voice_id not in VOICES:
            raise ValueError(f"unknown voice: {voice_id}")
        if not text.strip():
            raise ValueError("text is empty")
        if len(text) > MAX_TEXT_LENGTH:
            raise ValueError(f"text is longer than {MAX_TEXT_LENGTH} characters")
        if not 0.5 <= speed <= 2.0:
            raise ValueError("speed must be between 0.5 and 2.0")
        if not -12.0 <= pitch <= 12.0:
            raise ValueError("pitch must be between -12 and 12 semitones")

        base = VOICES[voice_id]["base_pitch"]
        samples = []
        for index, char in enumerate(text, start=1):
            samples.extend(_segment(char, base, speed, pitch))
            progress(index / len(text))
        _write_wav(output_path, samples)
        return {
            "path": output_path,
            "sample_rate": SAMPLE_RATE,
            "duration_sec": round(len(samples) / SAMPLE_RATE, 3),
        }


def _segment(char, base_pitch, speed, pitch):
    if char.isspace():
        return [0.0] * int(SAMPLE_RATE * SPACE_SEC / speed)
    if char in PAUSE_CHARS or not char.isalnum():
        return [0.0] * int(SAMPLE_RATE * PAUSE_SEC / speed)
    count = max(1, int(SAMPLE_RATE * TONE_SEC / speed))
    frequency = base_pitch * 2 ** ((ord(char) % 12 + pitch) / 12)
    return [
        0.3 * math.sin(2 * math.pi * frequency * i / SAMPLE_RATE) * math.sin(math.pi * i / count)
        for i in range(count)
    ]


def _write_wav(path, samples):
    pcm = array.array("h", (int(max(-1.0, min(1.0, s)) * 32767) for s in samples))
    if sys.byteorder == "big":
        pcm.byteswap()
    with wave.open(path, "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(SAMPLE_RATE)
        wav.writeframes(pcm.tobytes())
