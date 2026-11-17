import os
import sys
import tempfile
import unittest
import wave

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from main import Worker  # noqa: E402


class WorkerTests(unittest.TestCase):
    def setUp(self):
        self.events = []
        self.worker = Worker(self.events.append)
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.worker.handle({"id": 1, "method": "init", "params": {"engine": "debug"}})

    def synth(self, **params):
        base = {"text": "Hello, world!", "voice": "debug-a", "output_path": os.path.join(self.tmp.name, "out.wav")}
        base.update(params)
        return self.worker.handle({"id": 2, "method": "synthesize", "params": base})[0]

    def test_init_returns_voices(self):
        response, keep = self.worker.handle({"id": 3, "method": "list_voices"})
        self.assertTrue(keep)
        self.assertEqual({v["id"] for v in response["result"]["voices"]}, {"debug-a", "debug-b"})

    def test_synthesize_writes_valid_wav_and_reports_progress(self):
        response = self.synth()
        result = response["result"]
        with wave.open(result["path"], "rb") as wav:
            self.assertEqual(wav.getnchannels(), 1)
            self.assertEqual(wav.getsampwidth(), 2)
            self.assertEqual(wav.getframerate(), result["sample_rate"])
            self.assertGreater(wav.getnframes(), 0)
        self.assertGreater(result["duration_sec"], 0)
        progress = [e["progress"] for e in self.events if e.get("event") == "progress"]
        self.assertEqual(progress[-1], 1.0)

    def test_invalid_params_are_reported(self):
        self.assertEqual(self.synth(text="   ")["error"]["code"], "invalid_params")
        self.assertEqual(self.synth(speed=5)["error"]["code"], "invalid_params")
        self.assertEqual(self.synth(pitch=40)["error"]["code"], "invalid_params")
        self.assertEqual(self.synth(voice="missing")["error"]["code"], "invalid_params")
        self.assertEqual(self.synth(text="a" * 2001)["error"]["code"], "invalid_params")

    def test_missing_parameter_and_uninitialized_engine(self):
        fresh = Worker(lambda _: None)
        response, _ = fresh.handle({"id": 9, "method": "list_voices"})
        self.assertEqual(response["error"]["code"], "invalid_params")
        response, _ = self.worker.handle({"id": 10, "method": "synthesize", "params": {"voice": "debug-a"}})
        self.assertEqual(response["error"]["code"], "invalid_params")

    def test_shutdown_stops_loop(self):
        _, keep = self.worker.handle({"id": 4, "method": "shutdown"})
        self.assertFalse(keep)

    def test_unknown_method(self):
        response, keep = self.worker.handle({"id": 5, "method": "nope"})
        self.assertTrue(keep)
        self.assertEqual(response["error"]["code"], "unknown_method")


if __name__ == "__main__":
    unittest.main()
