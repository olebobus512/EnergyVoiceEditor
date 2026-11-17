# Energy Voice Editor

**An open-source synthetic voice and singing synthesis editor by Suharik Studios.**

> Create synthetic voices, dialogue, voiceovers and vocals for games and other creative projects.

## Voicebanks (EVBANK)

`Settings > Voicebank Manager` installs, checks, previews, exports and removes voice banks (`.evbank`), and packs your own from a folder. The format is described in `Documentation/EVBANK_SCHEMA.md`. **No synthesis engine can use a bank yet**: the manager says so for every bank. A test bank, `Documentation/examples/ExampleTone.evbank` (tones, not a voice), is included to try the manager. Full documentation and the roadmap: `Documentation/`.

## Extensions and downloadable voices

Settings → Extensions installs custom additive instruments and runs trusted Python note presets, with preview and Undo. Examples are in `examples/extensions/`. Settings → Voice catalogue downloads a verified Piper model or imports local ONNX/config pairs. Piper remains an optional external dependency. See [the implemented API and limits](Documentation/EXTENSIONS.md).

## Voices (current state)

The piano roll can play and export notes using a built-in electronic vowel guide voice. It follows note pitch, timing and the final vowel of the lyric, but does not synthesize consonants or use the selected speech voice or EVBANK. The Rust renderer mixes these notes with WAV clips and applies track mute, solo, volume and pan. It is not a neural or sample-based singing engine. Piano-roll key previews use the same Rust instrument renderer as timeline playback and export.

The voice list is built from every speech engine that is available on the machine:

- **Windows voices**: all voices installed in Windows (SAPI5), for example David, Zira, and Irina or Pavel with the Russian language pack. No installation needed. Speed is applied through the Windows rate scale (coarse steps). Pitch is best-effort and depends on the voice.
- **Piper (neural voices)**: much better quality, fully offline. Install `pip install piper-tts`, then download voice models (`.onnx` and `.onnx.json`) from <https://huggingface.co/rhasspy/piper-voices> and put them into the voices folder (Help > Open Voices Folder). Right-click the voice list and choose "Refresh Voice List" to pick up new models. Piper has no pitch control.
- **Debug tones**: only shown when no real voice is available. Not speech.

## 1. Project Overview

Energy Voice Editor is a desktop application designed for creating synthetic speech and singing vocals.

The primary goal is to provide game developers, content creators and musicians with a unified tool for generating and editing synthetic voices.

Unlike traditional singing synthesizers, Energy is intended to support both:

- **Voice Mode:** generating ordinary dialogue, narration and voiceovers.
- **Music Mode:** creating synthetic singing vocals using lyrics, musical notes and phoneme controls.

Both modes should share a common voice library and underlying audio infrastructure while using specialized synthesis engines.

Energy is being developed by **Suharik Studios**.

## 2. Core Principles

The project follows these principles:

- **Local First:** local synthesis should be available without an internet connection.
- **Hybrid Architecture:** support both local and cloud-based synthesis.
- **Synthetic Voices:** focus on fictional, stylized and synthetic voices rather than realistic voice imitation.
- **Modularity:** synthesis engines and voice models must be replaceable independently.
- **Extensibility:** allow future third-party engines and voice packs.
- **Cross-platform:** design the application with Windows and Linux compatibility in mind.
- **Open Source:** keep the core application source code publicly available, with the final license to be determined.

## 3. Technology Stack

### Desktop Application
- Tauri 2
- Vue 3
- TypeScript
- HTML5
- CSS3

### Backend
- Rust
- Tauri Commands and Events
- SQLite for application metadata and settings

### Machine Learning
- Python
- PyTorch
- ONNX Runtime
- Compatible open-source speech and singing synthesis models

### Audio
- WAV
- FLAC
- OGG
- Audio processing and conversion utilities

The frontend is responsible for application interaction and presentation. The Rust backend handles application logic, project management, engine orchestration and communication with external processes.

Python is used for machine learning inference and experimentation.

## 4. Architecture

The application should follow a modular architecture.

### 4.1 Frontend

The Vue application is responsible for:

- Application interaction.
- Project management interface.
- Voice selection.
- Synthesis settings.
- Audio playback.
- Communication with the Rust backend.

The visual design and interface layout are developed separately and will be supplied as an existing design or implementation.

Do not independently redesign or replace the interface.

### 4.2 Rust Core

The Rust backend is the central application layer.

Responsibilities:

- Project management.
- Voice Pack management.
- Engine discovery and initialization.
- Local and cloud engine selection.
- Python process management.
- Synthesis task orchestration.
- Audio file management.
- Settings and metadata storage.
- Error handling.

### 4.3 Python Runtime

Python is used as an independent machine learning runtime.

Responsibilities:

- Loading synthesis models.
- Running inference.
- Managing model resources.
- Processing synthesis requests.
- Returning generated audio.
- Reporting progress and errors.

Initially, Python should run as a separate process controlled by Rust.

Communication should use a documented JSON-based IPC protocol over stdin/stdout.

The protocol should support:

- Initialization.
- Synthesis requests.
- Progress notifications.
- Cancellation.
- Error reporting.
- Shutdown.

Do not couple the entire application to a single Python model or framework.

### 4.4 Local Engine

The Local Engine performs synthesis directly on the user's computer.

Requirements:

- No internet connection required.
- Support compatible local models.
- CPU inference fallback.
- GPU acceleration where supported.
- Model availability and compatibility checks.
- Resource management.

ONNX Runtime should be used for compatible exported models. PyTorch may be used when a model requires its original runtime.

### 4.5 Cloud Engine

The Cloud Engine provides access to remote synthesis services.

Requirements:

- Provider-independent API abstraction.
- Secure communication over HTTPS.
- Explicit user consent before sending text or project data.
- Request cancellation where supported.
- Error handling and timeout management.
- Clear indication that synthesis is taking place remotely.

Cloud synthesis must never happen silently.

### 4.6 Engine Manager

Engine Manager is responsible for selecting and coordinating synthesis engines.

Supported operating modes:

- `Local Only`
- `Cloud Only`
- `Auto (Local First)`

The Auto mode should prefer local synthesis when a compatible engine and model are available.

Cloud fallback must respect the user's privacy settings and require authorization.

## 5. Voice System

Energy should use an independent Voice Pack system.

Each Voice Pack contains:

- Unique identifier.
- Display name.
- Version.
- Description.
- Supported languages.
- Associated synthesis engine.
- Model files or model references.
- Configuration.
- Capabilities.
- License information.

Example structure:

```text
VoicePacks/
└── ExampleVoice/
    ├── manifest.json
    ├── model/
    ├── config/
    ├── assets/
    └── LICENSE
```

Voice Packs should be independent of the application release cycle.

The system should validate manifests, model compatibility and licensing metadata before loading a pack.

## 6. Synthesis Modes

### Voice Mode

Designed for ordinary synthetic speech.

Planned capabilities:

- Text-to-speech.
- Synthetic voice selection.
- Speech speed control.
- Pitch control where supported.
- Emotion controls where supported.
- Pause management.
- Audio preview.
- Batch generation.
- WAV export.

### Music Mode

Designed for synthetic singing.

Planned capabilities:

- Piano Roll.
- Lyrics editor.
- Phoneme editor.
- Note duration control.
- Pitch curves.
- Vibrato.
- Singing synthesis.
- Vocal track export.

Voice Mode and Music Mode should share the application infrastructure but use separate specialized synthesis engines.

Music Mode is not part of the initial MVP.

## 7. Audio Processing

The audio subsystem should provide a unified interface for:

- Loading generated audio.
- Audio preview.
- Format conversion.
- Basic processing.
- Export.
- File validation.

The initial export format should be WAV.

FLAC and OGG support can be introduced later.

Audio processing must not be tightly coupled to a particular synthesis engine.

## 8. Project Management

Energy should support persistent projects.

Project data should include:

- Project metadata.
- Selected voices.
- Synthesis settings.
- Text and generated dialogue.
- Audio file references.
- Generation history.

Use JSON for project serialization and SQLite for application-level metadata.

Generated audio should be stored separately from project metadata.

The project format should be versioned to support future migrations.

## 9. Suggested Repository Structure

```text
Energy-Voice-Editor/
│
├── apps/
│   └── desktop/
│       ├── src/
│       │   ├── components/
│       │   ├── views/
│       │   ├── stores/
│       │   ├── composables/
│       │   └── services/
│       │
│       └── src-tauri/
│           └── src/
│               ├── commands/
│               ├── core/
│               ├── engines/
│               ├── audio/
│               ├── projects/
│               └── main.rs
│
├── engines/
│   ├── python-runtime/
│   │   ├── engines/
│   │   ├── inference/
│   │   ├── models/
│   │   └── main.py
│   │
│   └── engine-protocol/
│
├── packages/
│   ├── shared-types/
│   ├── voice-pack-schema/
│   └── audio-utils/
│
├── cloud/
│   └── api/
│
├── docs/
├── tests/
├── README.md
└── LICENSE
```

This structure is a proposed starting point. Directories should be created when their corresponding functionality is implemented.

## 10. Development Roadmap

### Phase 1: Foundation
- Initialize Tauri 2 and Vue 3.
- Configure the Rust backend.
- Establish the project structure.
- Implement application settings.
- Implement Python Worker communication.
- Define the synthesis IPC protocol.

### Phase 2: Local Voice Engine
- Integrate an existing synthetic TTS model.
- Implement model loading.
- Implement voice selection.
- Implement text synthesis.
- Generate WAV files.
- Implement audio playback.

### Phase 3: Voice Editor
- Add speech speed controls.
- Add pitch controls where supported.
- Add emotion controls where supported.
- Add pause management.
- Implement batch generation.
- Implement generation history.

### Phase 4: Hybrid Synthesis
- Implement Cloud Engine abstraction.
- Add remote synthesis API integration.
- Implement Local First selection.
- Add provider configuration.
- Implement privacy controls and error handling.

### Phase 5: Music Mode
- Implement Piano Roll.
- Implement lyrics and phoneme editing.
- Integrate a specialized singing synthesis engine.
- Implement vocal rendering.

### Phase 6: Ecosystem
- Expand Voice Pack support.
- Introduce third-party engine extensions.
- Develop a model management system.
- Improve cross-platform compatibility.
- Explore Unreal Engine integration.

## 11. Development Guidelines for AI Coding Agents

When working on this project, follow these rules:

1. Read the existing project structure before making changes.
2. Do not replace existing technologies without a clear technical reason.
3. Do not introduce unnecessary dependencies.
4. Keep Rust, Python and Vue responsibilities separated.
5. Do not implement machine learning inference directly in the frontend.
6. Do not hardcode a specific synthesis model into the application core.
7. Keep synthesis engines modular and replaceable.
8. Use asynchronous operations for long-running synthesis tasks.
9. Implement proper error handling and logging.
10. Validate external input and generated file paths.
11. Avoid blocking the main application thread.
12. Write maintainable, documented code.
13. Add tests for important functionality.
14. Never claim that a feature works without testing it.
15. Do not implement the entire roadmap in one operation.
16. Do not redesign the application's interface independently of the supplied design.

Before implementing a new feature, inspect the relevant existing modules and explain the intended changes when the task is architecturally significant.

Prefer small, testable implementation steps.

## 12. Initial MVP Definition

The first functional prototype should demonstrate the complete local synthesis pipeline.

Required functionality:

1. Launch the desktop application.
2. Initialize the Rust Core.
3. Start the Python Worker.
4. Load a compatible local synthetic TTS model.
5. Accept text input.
6. Select an available synthetic voice.
7. Send a synthesis request.
8. Receive the generated WAV file.
9. Play the generated audio.
10. Save the result.

The initial prototype does not require:

- Music Mode.
- Cloud synthesis.
- Voice cloning.
- Plugin marketplace.
- Advanced audio editing.
- Third-party Voice Pack installation.

The priority is to establish a stable, modular foundation before expanding functionality.

---

**Energy Voice Editor**
 
*Developed by Suharik Studios.*

*Create voices. Build worlds. Bring characters to life.*
