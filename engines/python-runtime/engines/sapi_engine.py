"""Windows system voices (SAPI5) as a speech engine.

Uses only Windows PowerShell and System.Speech, so it needs no Python packages and works offline.
It lists every voice installed in Windows (for example Microsoft David, Zira, Irina, Pavel, depending on the
language packs). The quality is that of the Windows desktop voices, not of neural models.
"""

import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
from xml.sax.saxutils import escape

from .common import validate_params, wav_info

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "sapi_synth.ps1")
TIMEOUT_SEC = 90
ID_PREFIX = "sapi:"


def sapi_rate(speed):
    """Maps a speed factor (0.5..2.0) to the SAPI rate scale. SAPI documents -10 as 1/3x and +10 as 3x."""
    return max(-10, min(10, round(10 * math.log(speed) / math.log(3))))


def display_name(name, culture):
    short = name.removeprefix("Microsoft ").removesuffix(" Desktop")
    return f"{short} ({culture})" if culture else short


def build_ssml(text, culture, pitch):
    lang = culture or "en-US"
    semitones = f"{int(round(pitch)):+d}st"
    return (
        '<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" '
        f'xml:lang="{escape(lang)}"><prosody pitch="{semitones}">{escape(text)}</prosody></speak>'
    )


def run_powershell(args, timeout):
    """Default runner: returns (returncode, stdout_bytes, stderr_bytes)."""
    command = ["powershell", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", SCRIPT, *args]
    flags = 0x0800_0000 if sys.platform == "win32" else 0  # CREATE_NO_WINDOW
    done = subprocess.run(command, capture_output=True, timeout=timeout, creationflags=flags)
    return done.returncode, done.stdout, done.stderr


class SapiEngine:
    name = "sapi"
    label = "Windows voices"
    fallback = False

    def __init__(self, runner=None):
        self._runner = runner or run_powershell
        self._voices = None

    def configure(self, voices_dir):
        """Nothing to configure: the voices live in Windows."""

    def unavailable_reason(self):
        if sys.platform != "win32":
            return "only available on Windows"
        if shutil.which("powershell") is None:
            return "powershell.exe was not found"
        return None

    def list_voices(self, refresh=False):
        if self._voices is None or refresh:
            self._voices = self._query()
        return list(self._voices)

    def _query(self):
        code, out, err = self._runner(["-Action", "list"], 30)
        if code != 0:
            raise RuntimeError(f"could not list Windows voices: {_tail(err)}")
        try:
            data = json.loads(out.decode("utf-8-sig").strip() or "[]")
        except ValueError as exc:
            raise RuntimeError(f"could not read the Windows voice list: {exc}") from exc
        if isinstance(data, dict):
            data = [data]
        voices = []
        for item in data:
            name = item.get("name")
            if not name:
                continue
            culture = item.get("culture") or ""
            voices.append(
                {
                    "id": ID_PREFIX + name,
                    "name": display_name(name, culture),
                    "languages": [culture] if culture else ["any"],
                    "engine": self.name,
                    "gender": str(item.get("gender") or "").lower(),
                    "supportsPitch": True,
                }
            )
        return voices

    def synthesize(self, text, voice_id, output_path, speed, pitch, progress):
        validate_params(text, speed, pitch)
        voice = next((item for item in self.list_voices() if item["id"] == voice_id), None)
        if voice is None:
            raise ValueError(f"unknown voice: {voice_id}")
        culture = voice["languages"][0] if voice["languages"][0] != "any" else ""
        job = {
            "voice": voice_id[len(ID_PREFIX):],
            "rate": sapi_rate(speed),
            "text": text,
            "output": output_path,
            "ssml": build_ssml(text, culture, pitch) if round(pitch) != 0 else None,
        }
        progress(0.1)
        with tempfile.TemporaryDirectory() as tmp:
            job_file = os.path.join(tmp, "job.json")
            with open(job_file, "w", encoding="utf-8") as handle:
                json.dump(job, handle)
            code, out, err = self._runner(["-Action", "speak", "-JobFile", job_file], TIMEOUT_SEC)
        if code != 0:
            raise RuntimeError(f"Windows speech synthesis failed: {_tail(err)}")
        if job["ssml"] and not _pitch_applied(out):
            print("[sapi] this voice ignored the pitch setting", file=sys.stderr, flush=True)
        sample_rate, duration = wav_info(output_path)
        progress(1.0)
        return {"path": output_path, "sample_rate": sample_rate, "duration_sec": duration}


def _pitch_applied(out):
    try:
        return bool(json.loads(out.decode("utf-8-sig").strip() or "{}").get("pitchApplied"))
    except ValueError:
        return False


def _tail(data):
    text = data.decode("utf-8", errors="replace").strip()
    return text[-400:] if text else "no details"
