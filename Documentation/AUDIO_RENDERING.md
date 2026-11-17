# Audio rendering and export

Status: **offline mix implemented**; synthesis stages not started.

## Today
`render_mix` takes sample rate (8000–192000), channels (1/2), bit depth (16/24), start time and the track list, and writes a WAV. Preview writes into the generated-audio folder; export writes to the chosen file. Preview and export use the same function.

## Required by the prompt, not yet done
Export range (selection, loop), selected track only, normalisation, additional formats through codecs. The requirement that **preview and final render must not differ** holds today because they share one function; it must keep holding when the realtime engine arrives (same renderer, different sink).
