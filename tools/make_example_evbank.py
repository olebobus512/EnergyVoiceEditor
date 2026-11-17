"""Builds Documentation/examples/ExampleTone.evbank: a tiny TEST voice bank made of synthesized tones.

It is not a voice. It exists so the Voicebank Manager, the loader and the tests have a real .evbank to work
with, and it is written by code that is independent of the Rust packer on purpose: if both agree, the format
description is precise enough to implement twice.

Usage:  python tools/make_example_evbank.py [output.evbank]
"""

import hashlib
import json
import math
import os
import struct
import sys
import wave
import zipfile

RATE = 22050
VOWELS = {  # rough formant peaks in Hz, only to make five audibly different test tones
    "a": (800, 1200),
    "i": (300, 2300),
    "u": (320, 800),
    "e": (500, 1800),
    "o": (500, 900),
}
BASE_HZ = 220.0  # MIDI 57


def tone(f1, f2, seconds=0.6):
    count = int(RATE * seconds)
    out = []
    for n in range(count):
        t = n / RATE
        value = 0.0
        for k in range(1, 30):
            freq = BASE_HZ * k
            if freq > RATE / 2.2:
                break
            weight = math.exp(-((freq - f1) / 250) ** 2) + 0.6 * math.exp(-((freq - f2) / 350) ** 2) + 0.05 / k
            value += weight * math.sin(2 * math.pi * freq * t)
        attack = min(1.0, t / 0.02)
        release = min(1.0, (seconds - t) / 0.05)
        out.append(value * attack * release)
    peak = max(abs(v) for v in out) or 1.0
    return [int(32000 * v / peak * 0.6) for v in out]


def wav_bytes(samples):
    import io

    buffer = io.BytesIO()
    with wave.open(buffer, "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(RATE)
        handle.writeframes(b"".join(struct.pack("<h", s) for s in samples))
    return buffer.getvalue()


def main():
    out_path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        os.path.dirname(os.path.abspath(__file__)), "..", "Documentation", "examples", "ExampleTone.evbank"
    )
    files = {}
    all_samples = []
    for name, (f1, f2) in VOWELS.items():
        samples = tone(f1, f2)
        files[f"Audio/Samples/{name}.wav"] = wav_bytes(samples)
        all_samples += samples + [0] * int(RATE * 0.1)
    files["Resources/preview.wav"] = wav_bytes(all_samples)
    files["Config/synthesis.json"] = json.dumps({"baseMidi": 57, "note": "test tones, not a voice"}, indent=2).encode()

    manifest = {
        "format": "evbank",
        "formatVersion": 1,
        "voiceId": "example-tone",
        "voiceName": "Example Tone (test bank)",
        "version": "1.0.0",
        "author": "Suharik Studios",
        "description": "Five synthesized test tones (a i u e o). Not a voice: it exists to test the voicebank system.",
        "languages": ["en"],
        "synthesisTypes": ["sample", "singing"],
        "supportedBackends": ["eve-classic"],
        "phonemizers": [],
        "audioConfiguration": {"sampleRate": RATE, "channels": 1, "bitDepth": 16},
        "pitchRange": {"minMidi": 45, "maxMidi": 69},
        "license": {"name": "CC0-1.0", "commercialUse": True},
        "files": sorted(files),
        "checksums": {name: hashlib.sha256(data).hexdigest() for name, data in sorted(files.items())},
        "preview": "Resources/preview.wav",
        "tags": ["test", "tones"],
    }
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    with zipfile.ZipFile(out_path, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("manifest.json", json.dumps(manifest, indent=2))
        for name, data in sorted(files.items()):
            archive.writestr(name, data)
    print(f"wrote {os.path.abspath(out_path)} ({os.path.getsize(out_path)} bytes)")


if __name__ == "__main__":
    main()
