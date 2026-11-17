"""Helpers shared by the speech engines."""

import wave

MAX_TEXT_LENGTH = 2000


def validate_params(text, speed, pitch):
    """Raises ValueError with a readable message when synthesis parameters are out of range."""
    if not text.strip():
        raise ValueError("text is empty")
    if len(text) > MAX_TEXT_LENGTH:
        raise ValueError(f"text is longer than {MAX_TEXT_LENGTH} characters")
    if not 0.5 <= speed <= 2.0:
        raise ValueError("speed must be between 0.5 and 2.0")
    if not -12.0 <= pitch <= 12.0:
        raise ValueError("pitch must be between -12 and 12 semitones")


def wav_info(path):
    """Returns (sample_rate, duration_sec) of a PCM WAV file. Raises RuntimeError if it cannot be read."""
    try:
        with wave.open(path, "rb") as wav:
            rate = wav.getframerate()
            frames = wav.getnframes()
    except (OSError, wave.Error, EOFError) as exc:
        raise RuntimeError(f"engine produced an unreadable WAV file: {exc}") from exc
    if rate <= 0 or frames <= 0:
        raise RuntimeError("engine produced an empty audio file")
    return rate, round(frames / rate, 3)
