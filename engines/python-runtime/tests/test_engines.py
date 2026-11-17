import json
import os
import sys
import tempfile
import unittest
from unittest.mock import patch
import wave

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from engines.piper_engine import PiperEngine, describe_model  # noqa: E402
from engines.sapi_engine import SapiEngine, build_ssml, display_name, sapi_rate  # noqa: E402
from main import Worker  # noqa: E402


def write_wav(path, seconds=0.2, rate=22050):
    with wave.open(path, "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(rate)
        wav.writeframes(b"\x00\x00" * int(rate * seconds))


class FakeSapiRunner:
    """Stands in for powershell.exe: answers `list` and writes a WAV for `speak`."""

    def __init__(self, voices=None, fail=False, pitch_applied=True):
        self.voices = voices if voices is not None else [
            {"name": "Microsoft David Desktop", "culture": "en-US", "gender": "Male", "age": "Adult"},
            {"name": "Microsoft Irina Desktop", "culture": "ru-RU", "gender": "Female", "age": "Adult"},
        ]
        self.fail = fail
        self.pitch_applied = pitch_applied
        self.jobs = []
        self.list_calls = 0

    def __call__(self, args, timeout):
        if args[1] == "list":
            self.list_calls += 1
            return 0, b"\xef\xbb\xbf" + json.dumps(self.voices).encode("utf-8"), b""
        if self.fail:
            return 1, b"", b"boom"
        with open(args[3], encoding="utf-8") as handle:
            job = json.load(handle)
        self.jobs.append(job)
        write_wav(job["output"])
        return 0, json.dumps({"pitchApplied": self.pitch_applied}).encode(), b""


class SapiScriptTests(unittest.TestCase):
    """The PowerShell script cannot run on this machine, so guard against the mistakes that already happened."""

    def setUp(self):
        path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "engines", "sapi_synth.ps1")
        with open(path, encoding="utf-8") as handle:
            self.text = handle.read()

    def test_variables_do_not_collide_with_parameters(self):
        import re

        block = re.search(r"param\((.*?)\n\)", self.text, flags=re.S).group(1)
        params = [name.lower() for name in re.findall(r"\]\s*\$(\w+)", block)]
        self.assertEqual(sorted(params), ["action", "jobfile"])
        assigned = {name.lower() for name in re.findall(r"^\s*\$(\w+)\s*=", self.text, flags=re.M)}
        self.assertEqual(assigned & set(params), set(), "PowerShell variables are case-insensitive: $job would overwrite $Job")

    def test_python_passes_the_parameter_the_script_declares(self):
        runner = FakeSapiRunner()
        with tempfile.TemporaryDirectory() as tmp:
            seen = []
            original = runner.__call__

            def spy(args, timeout):
                seen.append(list(args))
                return original(args, timeout)

            SapiEngine(spy).synthesize("hi", "sapi:Microsoft David Desktop", os.path.join(tmp, "o.wav"), 1.0, 0.0, lambda _: None)
        speak = next(args for args in seen if args[1] == "speak")
        self.assertEqual(speak[2], "-JobFile")
        self.assertIn("[string]$JobFile", self.text)


class SapiEngineTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.out = os.path.join(self.tmp.name, "out.wav")

    def test_rate_mapping_follows_sapi_scale(self):
        self.assertEqual(sapi_rate(1.0), 0)
        self.assertEqual(sapi_rate(3.0), 10)
        self.assertEqual(sapi_rate(2.0), 6)
        self.assertEqual(sapi_rate(0.5), -6)
        self.assertTrue(all(-10 <= sapi_rate(x / 10) <= 10 for x in range(5, 21)))

    def test_display_names_are_shortened(self):
        self.assertEqual(display_name("Microsoft David Desktop", "en-US"), "David (en-US)")
        self.assertEqual(display_name("Custom Voice", ""), "Custom Voice")

    def test_ssml_escapes_text_and_sets_semitones(self):
        ssml = build_ssml("a < b & c", "en-US", 3)
        self.assertIn("a &lt; b &amp; c", ssml)
        self.assertIn('pitch="+3st"', ssml)
        self.assertIn('pitch="-5st"', build_ssml("x", "", -5))

    def test_lists_voices_and_caches(self):
        runner = FakeSapiRunner()
        engine = SapiEngine(runner)
        voices = engine.list_voices()
        self.assertEqual([v["id"] for v in voices], ["sapi:Microsoft David Desktop", "sapi:Microsoft Irina Desktop"])
        self.assertEqual(voices[1]["languages"], ["ru-RU"])
        self.assertEqual(voices[1]["gender"], "female")
        engine.list_voices()
        self.assertEqual(runner.list_calls, 1)
        engine.list_voices(refresh=True)
        self.assertEqual(runner.list_calls, 2)

    def test_single_voice_json_object_is_accepted(self):
        engine = SapiEngine(FakeSapiRunner(voices={"name": "Only", "culture": "en-GB"}))
        self.assertEqual(len(engine.list_voices()), 1)

    def test_synthesize_writes_wav_and_builds_job(self):
        runner = FakeSapiRunner()
        engine = SapiEngine(runner)
        progress = []
        result = engine.synthesize("Привет, мир", "sapi:Microsoft Irina Desktop", self.out, 2.0, 0.0, progress.append)
        self.assertEqual(result["sample_rate"], 22050)
        self.assertAlmostEqual(result["duration_sec"], 0.2, places=2)
        job = runner.jobs[0]
        self.assertEqual(job["voice"], "Microsoft Irina Desktop")
        self.assertEqual(job["rate"], 6)
        self.assertEqual(job["text"], "Привет, мир")
        self.assertIsNone(job["ssml"])
        self.assertEqual(progress[-1], 1.0)

    def test_pitch_uses_ssml(self):
        runner = FakeSapiRunner()
        SapiEngine(runner).synthesize("Hello", "sapi:Microsoft David Desktop", self.out, 1.0, 4.0, lambda _: None)
        self.assertIn('pitch="+4st"', runner.jobs[0]["ssml"])
        self.assertIn('xml:lang="en-US"', runner.jobs[0]["ssml"])

    def test_errors_are_readable(self):
        engine = SapiEngine(FakeSapiRunner())
        with self.assertRaises(ValueError):
            engine.synthesize("x", "sapi:Nobody", self.out, 1.0, 0.0, lambda _: None)
        with self.assertRaises(ValueError):
            engine.synthesize("  ", "sapi:Microsoft David Desktop", self.out, 1.0, 0.0, lambda _: None)
        with self.assertRaisesRegex(RuntimeError, "boom"):
            SapiEngine(FakeSapiRunner(fail=True)).synthesize("x", "sapi:Microsoft David Desktop", self.out, 1.0, 0.0, lambda _: None)

    @unittest.skipIf(sys.platform == "win32", "checks the non-Windows answer")
    def test_unavailable_outside_windows(self):
        self.assertIn("Windows", SapiEngine().unavailable_reason())


FAKE_PIPER = r'''
import sys, wave
args = sys.argv[1:]

if "--help" in args:
    print("usage: piper -m MODEL -f FILE --length_scale X" if STYLE == "underscore" else "usage: piper -m MODEL -f FILE --length-scale X")
    sys.exit(0)
model = args[args.index("-m") + 1]
out = args[args.index("-f") + 1]
flag = "--length_scale" if STYLE == "underscore" else "--length-scale"
scale = float(args[args.index(flag) + 1])
text = sys.stdin.buffer.read().decode("utf-8")
with open(out + ".args", "w", encoding="utf-8") as handle:
    handle.write(f"{model}|{scale}|{text}")
with wave.open(out, "wb") as wav:
    wav.setnchannels(1); wav.setsampwidth(2); wav.setframerate(22050)
    wav.writeframes(b"\x00\x00" * 4410)
'''


class PiperEngineTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.voices_dir = os.path.join(self.tmp.name, "voices")
        self.piper_dir = os.path.join(self.voices_dir, "piper")
        os.makedirs(os.path.join(self.piper_dir, "pack"))
        for name in ("ru_RU-irina-medium.onnx", os.path.join("pack", "en_US-lessac-high.onnx"), "notes.txt"):
            open(os.path.join(self.piper_dir, name), "wb").close()
        self.out = os.path.join(self.tmp.name, "out.wav")

    def fake_command(self, style):
        script = os.path.join(self.tmp.name, f"fake_piper_{style}.py")
        with open(script, "w", encoding="utf-8") as handle:
            handle.write(f'STYLE = "{style}"\n' + FAKE_PIPER)
        return [sys.executable, script]

    def test_model_names_are_described(self):
        self.assertEqual(describe_model("ru_RU-irina-medium"), ("Irina (ru_RU, medium)", "ru_RU"))
        self.assertEqual(describe_model("en_US-ryan_special-high"), ("Ryan Special (en_US, high)", "en_US"))
        self.assertEqual(describe_model("custom"), ("custom", "any"))

    def test_lists_models_as_voices(self):
        engine = PiperEngine(self.voices_dir, command=self.fake_command("dash"))
        voices = engine.list_voices()
        self.assertEqual([v["id"] for v in voices], ["piper:pack/en_US-lessac-high", "piper:ru_RU-irina-medium"])
        self.assertFalse(voices[0]["supportsPitch"])

    def test_no_models_means_no_voices(self):
        empty = os.path.join(self.tmp.name, "empty")
        self.assertEqual(PiperEngine(empty, command=self.fake_command("dash")).list_voices(), [])

    def test_synthesize_with_both_flag_styles(self):
        for style in ("dash", "underscore"):
            with self.subTest(style=style):
                engine = PiperEngine(self.voices_dir, command=self.fake_command(style))
                result = engine.synthesize("Hello\nthere", "piper:ru_RU-irina-medium", self.out, 2.0, 0.0, lambda _: None)
                self.assertEqual(result["sample_rate"], 22050)
                self.assertAlmostEqual(result["duration_sec"], 0.2, places=2)
                with open(self.out + ".args", encoding="utf-8") as handle:
                    model, scale, text = handle.read().split("|")
                self.assertTrue(model.endswith("ru_RU-irina-medium.onnx"))
                self.assertAlmostEqual(float(scale), 0.5, places=3)
                self.assertEqual(text, "Hello there")
                os.remove(self.out)
                os.remove(self.out + ".args")

    def test_rejects_unknown_and_escaping_voices(self):
        engine = PiperEngine(self.voices_dir, command=self.fake_command("dash"))
        for voice in ("piper:missing", "piper:../../etc/passwd", "sapi:x"):
            with self.assertRaises(ValueError):
                engine.synthesize("x", voice, self.out, 1.0, 0.0, lambda _: None)

    def test_failure_is_reported(self):
        engine = PiperEngine(self.voices_dir, command=[sys.executable, "-c", "import sys; sys.stderr.write('bad model'); sys.exit(3)"])
        with self.assertRaisesRegex(RuntimeError, "bad model"):
            engine.synthesize("x", "piper:ru_RU-irina-medium", self.out, 1.0, 0.0, lambda _: None)


class WorkerEngineTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    @patch.object(SapiEngine, "list_voices", return_value=[])
    def test_debug_fallback_is_used_when_nothing_else_has_voices(self, _list_voices):
        worker = Worker(lambda _: None)
        response, _ = worker.handle({"id": 1, "method": "init", "params": {"engines": ["sapi", "piper", "debug"], "voices_dir": self.tmp.name}})
        ids = {v["id"] for v in response["result"]["voices"]}
        self.assertEqual(ids, {"debug-a", "debug-b"})

    def test_debug_voices_are_hidden_when_real_voices_exist(self):
        worker = Worker(lambda _: None)
        worker.handle({"id": 1, "method": "init", "params": {"engines": ["piper", "debug"], "voices_dir": self.tmp.name}})
        os.makedirs(os.path.join(self.tmp.name, "piper"))
        open(os.path.join(self.tmp.name, "piper", "en_US-test-low.onnx"), "wb").close()
        worker.engines["piper"]._command_override = [sys.executable]
        response, _ = worker.handle({"id": 2, "method": "list_voices", "params": {"refresh": True}})
        self.assertEqual([v["id"] for v in response["result"]["voices"]], ["piper:en_US-test-low"])

    def test_unknown_engine_and_voice(self):
        worker = Worker(lambda _: None)
        response, _ = worker.handle({"id": 1, "method": "init", "params": {"engines": ["nope"]}})
        self.assertEqual(response["error"]["code"], "invalid_params")
        worker.handle({"id": 2, "method": "init", "params": {"engine": "debug"}})
        response, _ = worker.handle({"id": 3, "method": "synthesize", "params": {"text": "hi", "voice": "ghost", "output_path": os.path.join(self.tmp.name, "x.wav")}})
        self.assertEqual(response["error"]["message"], "unknown voice: ghost")

    def test_legacy_debug_voice_id_still_synthesizes(self):
        worker = Worker(lambda _: None)
        worker.handle({"id": 1, "method": "init", "params": {"engine": "debug"}})
        out = os.path.join(self.tmp.name, "legacy.wav")
        response, _ = worker.handle({"id": 2, "method": "synthesize", "params": {"text": "hi", "voice": "debug-a", "output_path": out}})
        self.assertGreater(response["result"]["duration_sec"], 0)

    def test_voice_metadata_fields(self):
        worker = Worker(lambda _: None)
        response, _ = worker.handle({"id": 1, "method": "init", "params": {"engine": "debug"}})
        voice = response["result"]["voices"][0]
        self.assertEqual({"id", "name", "languages", "engine", "gender", "supportsPitch"}, set(voice))


if __name__ == "__main__":
    unittest.main()
