# Voicebank System

**Purpose.** Package, install, check and describe voices independent of the synthesis technique.

**Features.** Implemented: the EVBANK format, loader, atomic install, verify, export, uninstall, preview, packer, manager UI. Not implemented: updates from a repository, dependency auto-install.

**Inputs.** `.evbank` archives, unpacked folders. **Outputs.** Installed banks, status, voice information.

**Scenario.** A user imports a bank; the manager verifies it, shows author, licence, languages and which engines can use it; the user previews and uses it on a track (the last step waits for a backend).

**Interaction.** Backends read banks as data; the project stores a reference (id, version, manifest checksum), not the bank.

**Approach and independence.** Our own zip-based container with a strict manifest (EVBANK_SCHEMA.md). No external bank format is read or imitated.

**Limits.** Checksums detect damage, not authorship (no signatures yet). Banks are only as legal as their recordings: the licence field is informational.

**Performance.** Verification reads every file once; large banks take seconds to minutes and run off the UI thread.

**Tests.** The EVBANK test suite in `src-tauri/src/evbank/` includes hostile archives (see SECURITY.md).
