"""Energy Voice Editor Python worker.

Reads one JSON request per line from stdin and writes JSON responses to stdout.
Progress notifications are written as {"event": "progress", ...} lines.
Logs go to stderr so they never corrupt the protocol stream.

Request:  {"id": 1, "method": "init|list_voices|synthesize|shutdown", "params": {...}}
Response: {"id": 1, "result": {...}}  or  {"id": 1, "error": {"code": "...", "message": "..."}}

Methods
  init         params: {"engines": ["sapi", "piper", "debug"], "voices_dir": "<folder>"}
               `engine` (a single name) is accepted instead of `engines`. All listed engines are loaded.
               result: {"engines": [...], "voices": [...]}
  list_voices  params: {"refresh": false}. `refresh` re-scans the engines (new Piper models, new Windows voices).
               result: {"voices": [{"id", "name", "languages", "engine", "gender", "supportsPitch"}, ...]}
  synthesize   params: {"text", "voice", "output_path", "speed", "pitch"}
               result: {"path", "sample_rate", "duration_sec"}
  shutdown     result: {}

Voice ids are unique across engines (`sapi:<Windows voice>`, `piper:<model>`, `debug-a`). The worker routes a
synthesis request to the engine that listed the voice. Engines flagged `fallback` (the debug tone generator) only
contribute voices when no other engine has any, so test tones never show up next to real voices.
"""

import json
import os
import sys
import traceback

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from engines.debug_engine import DebugEngine  # noqa: E402
from engines.piper_engine import PiperEngine  # noqa: E402
from engines.sapi_engine import SapiEngine  # noqa: E402

ENGINES = {"sapi": SapiEngine, "piper": PiperEngine, "debug": DebugEngine}
DEFAULT_ENGINES = ["sapi", "piper", "debug"]


def default_voices_dir():
    return os.environ.get("ENERGY_VOICES_DIR") or os.path.join(os.path.dirname(os.path.abspath(__file__)), "voices")


class Worker:
    def __init__(self, emit):
        self.emit = emit
        self.engines = {}
        self.voice_owner = {}

    def handle(self, request):
        """Returns (response, keep_running)."""
        req_id = request.get("id")
        method = request.get("method")
        params = request.get("params") or {}
        try:
            if method == "init":
                self._init(params)
                voices = self._voices(refresh=False)
                return {"id": req_id, "result": {"engines": list(self.engines), "voices": voices}}, True
            if method == "list_voices":
                self._require_engines()
                return {"id": req_id, "result": {"voices": self._voices(refresh=bool(params.get("refresh")))}}, True
            if method == "synthesize":
                self._require_engines()
                progress = lambda value: self.emit({"event": "progress", "id": req_id, "progress": value})
                voice_id = params["voice"]
                engine = self._engine_for(voice_id)
                result = engine.synthesize(
                    text=params["text"],
                    voice_id=voice_id,
                    output_path=params["output_path"],
                    speed=float(params.get("speed", 1.0)),
                    pitch=float(params.get("pitch", 0.0)),
                    progress=progress,
                )
                return {"id": req_id, "result": result}, True
            if method == "shutdown":
                return {"id": req_id, "result": {}}, False
            return _error(req_id, "unknown_method", f"unknown method: {method}"), True
        except KeyError as exc:
            return _error(req_id, "invalid_params", f"missing parameter: {exc.args[0]}"), True
        except ValueError as exc:
            return _error(req_id, "invalid_params", str(exc)), True
        except Exception as exc:  # keep the worker alive on unexpected failures
            traceback.print_exc(file=sys.stderr)
            return _error(req_id, "internal", str(exc)), True

    def _init(self, params):
        if params.get("engines"):
            names = list(params["engines"])
        elif params.get("engine"):
            names = [params["engine"]]
        else:
            names = DEFAULT_ENGINES
        voices_dir = params.get("voices_dir") or default_voices_dir()
        engines = {}
        for name in names:
            if name not in ENGINES:
                raise ValueError(f"unknown engine: {name}")
            engine = ENGINES[name]()
            reason = engine.unavailable_reason()
            if reason:
                print(f"[engine] {name} is not available: {reason}", file=sys.stderr, flush=True)
                continue
            engine.configure(voices_dir)
            engines[name] = engine
        if not engines:
            raise ValueError("no speech engine is available")
        self.engines = engines
        self.voice_owner = {}

    def _voices(self, refresh):
        listed = {}
        for name, engine in self.engines.items():
            try:
                listed[name] = engine.list_voices(refresh=refresh)
            except Exception as exc:  # one broken engine must not hide the others
                print(f"[engine] {name} could not list voices: {exc}", file=sys.stderr, flush=True)
                listed[name] = []
        has_real_voices = any(voices for name, voices in listed.items() if not self.engines[name].fallback)
        voices, owner = [], {}
        for name, engine_voices in listed.items():
            if self.engines[name].fallback and has_real_voices:
                continue
            for voice in engine_voices:
                voices.append(voice)
                owner[voice["id"]] = name
        self.voice_owner = owner
        return voices

    def _engine_for(self, voice_id):
        if voice_id not in self.voice_owner:
            self._voices(refresh=True)  # the voice may have been added since the last listing
        name = self.voice_owner.get(voice_id)
        if name is None:
            raise ValueError(f"unknown voice: {voice_id}")
        return self.engines[name]

    def _require_engines(self):
        if not self.engines:
            raise ValueError("engine is not initialized; send 'init' first")


def _error(req_id, code, message):
    return {"id": req_id, "error": {"code": code, "message": message}}


def main():
    sys.stdin.reconfigure(encoding="utf-8")
    out = sys.stdout

    def emit(obj):
        out.write(json.dumps(obj) + "\n")
        out.flush()

    worker = Worker(emit)
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
        except json.JSONDecodeError as exc:
            emit(_error(None, "invalid_json", str(exc)))
            continue
        response, keep_running = worker.handle(request)
        emit(response)
        if not keep_running:
            break


if __name__ == "__main__":
    main()
