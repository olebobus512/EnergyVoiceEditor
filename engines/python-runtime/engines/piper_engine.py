"""Piper neural voices as a speech engine (https://github.com/OHF-Voice/piper1-gpl).

Piper is not bundled. Install it with `pip install piper-tts` and put voice models in the voices folder,
`<voices_dir>/piper/`. A model is a pair of files, e.g. `ru_RU-irina-medium.onnx` and
`ru_RU-irina-medium.onnx.json`. Voices can be downloaded from https://huggingface.co/rhasspy/piper-voices.
Each model file found in the folder becomes one selectable voice. Piper has no pitch control.
"""

import importlib.util
import os
import re
import shutil
import subprocess
import sys

from .common import validate_params, wav_info

ID_PREFIX = "piper:"
TIMEOUT_SEC = 120
NAME_PATTERN = re.compile(r"^(?P<lang>[a-z]{2,3}_[A-Z]{2})-(?P<name>.+)-(?P<quality>x_low|low|medium|high)$")


def describe_model(stem):
    """Returns (display name, language) for a model file name such as ru_RU-irina-medium."""
    match = NAME_PATTERN.match(stem)
    if not match:
        return stem, "any"
    name = match["name"].replace("_", " ").title()
    return f"{name} ({match['lang']}, {match['quality']})", match["lang"]


class PiperEngine:
    name = "piper"
    label = "Piper (neural voices)"
    fallback = False

    def __init__(self, voices_dir=None, command=None):
        self._root = None
        self._command_override = command
        self._flag_style = None
        self._hinted = False
        if voices_dir:
            self.configure(voices_dir)

    def configure(self, voices_dir):
        self._root = os.path.join(voices_dir, "piper") if voices_dir else None

    def unavailable_reason(self):
        return None

    def _command(self):
        if self._command_override:
            return list(self._command_override)
        exe = shutil.which("piper")
        if exe:
            return [exe]
        if importlib.util.find_spec("piper") is not None:
            return [sys.executable, "-m", "piper"]
        return None

    def _models(self):
        if not self._root or not os.path.isdir(self._root):
            return []
        found = []
        for folder, _dirs, files in os.walk(self._root):
            for file in sorted(files):
                if file.lower().endswith(".onnx"):
                    found.append(os.path.join(folder, file))
        return sorted(found)

    def list_voices(self, refresh=False):
        models = self._models()
        if not models:
            self._hint(f"no voice models found in {self._root} (see engines/python-runtime/engines/piper_engine.py)")
            return []
        if self._command() is None:
            self._hint("voice models were found but Piper is not installed: run  pip install piper-tts")
            return []
        voices = []
        for path in models:
            relative = os.path.relpath(path, self._root).replace(os.sep, "/")
            name, language = describe_model(os.path.splitext(os.path.basename(path))[0])
            voices.append(
                {
                    "id": ID_PREFIX + relative[: -len(".onnx")],
                    "name": name,
                    "languages": [language],
                    "engine": self.name,
                    "gender": "",
                    "supportsPitch": False,
                }
            )
        return voices

    def _hint(self, message):
        if not self._hinted:
            self._hinted = True
            print(f"[piper] {message}", file=sys.stderr, flush=True)

    def _length_flag(self, command):
        """Older Piper builds spell the option --length_scale, newer ones --length-scale. Ask the tool."""
        if self._flag_style is None:
            try:
                done = subprocess.run([*command, "--help"], capture_output=True, timeout=30)
                text = (done.stdout + done.stderr).decode("utf-8", errors="replace")
                self._flag_style = "underscore" if "--length_scale" in text else "dash"
            except (OSError, subprocess.SubprocessError):
                self._flag_style = "dash"
        return "--length_scale" if self._flag_style == "underscore" else "--length-scale"

    def synthesize(self, text, voice_id, output_path, speed, pitch, progress):
        validate_params(text, speed, pitch)
        if not voice_id.startswith(ID_PREFIX):
            raise ValueError(f"unknown voice: {voice_id}")
        if not self._root:
            raise RuntimeError("Piper voices folder is not configured")
        root = os.path.normpath(self._root)
        model = os.path.normpath(os.path.join(root, voice_id[len(ID_PREFIX):] + ".onnx"))
        try:
            inside = os.path.commonpath([root, model]) == root
        except ValueError:  # different drives
            inside = False
        if not inside or not os.path.isfile(model):
            raise ValueError(f"unknown voice: {voice_id}")
        command = self._command()
        if command is None:
            raise RuntimeError("Piper is not installed: run  pip install piper-tts")
        if round(pitch) != 0:
            print("[piper] Piper voices have no pitch control; the pitch setting was ignored", file=sys.stderr, flush=True)
        args = [*command, "-m", model, "-f", output_path, self._length_flag(command), f"{1.0 / speed:.4f}"]
        progress(0.1)
        done = subprocess.run(
            args,
            input=" ".join(text.split()).encode("utf-8"),
            capture_output=True,
            timeout=TIMEOUT_SEC,
        )
        if done.returncode != 0 or not os.path.isfile(output_path):
            details = (done.stderr or b"").decode("utf-8", errors="replace").strip()[-400:] or "no details"
            raise RuntimeError(f"Piper failed: {details}")
        sample_rate, duration = wav_info(output_path)
        progress(1.0)
        return {"path": output_path, "sample_rate": sample_rate, "duration_sec": duration}
