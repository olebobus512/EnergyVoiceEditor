# Clean Room documentation

Purpose: record what vocal editors do **from the outside**, and design Energy Voice Editor's own solutions. Nothing here is copied from, or derived from, closed source, proprietary voice data, internal formats, or decompiled software.

## Rules
1. Allowed sources: official documentation, public manuals, public demonstrations of the interface, public technical papers and open specifications, open-source projects under their licences.
2. Not allowed: decompiling or disassembling, reading proprietary files to learn their format, using proprietary voice recordings or models.
3. When something about a product's internals is unknown, the document says "unknown" and the design here is our own.
4. Each source used is listed in `Research/SOURCES.md` with what was taken from it.

## State
This iteration did **not** do web research. The specifications below describe generally known, user-visible behaviour of vocal editors and our own design for it. Where a point would need a source, it is marked `NEEDS SOURCE`. No claim here describes any product's internals.

| Folder | Content | State |
|---|---|---|
| Research | source list and open questions | started (`SOURCES.md`) |
| Specifications | one file per component | 5 of 17 written, the rest listed in `Specifications/INDEX.md` |
| Architecture | pointers to `../SYSTEM_ARCHITECTURE.md` | – |
| Algorithms | method comparison in `../DSP_ENGINE.md` | – |
| Compatibility | what we import/export and why | `IMPORT_EXPORT.md` |
