<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { t } from "../i18n";
import { instrumentPlugins, refreshInstruments } from "../extensions";
import { doc, mutate, newId, selectedTrackObject, type Note } from "../project";
const emit = defineEmits<{ close: [] }>();
const busy = ref(false);
const message = ref("");
const script = ref("");
const trusted = ref(false);
const result = ref<{ name: string; tempo?: number; notes: Omit<Note, "id">[] } | null>(null);
let source = "";
let targetId = "";
async function install() {
  try {
    const path = await open({ filters: [{ name: "Energy instrument", extensions: ["json"] }] });
    if (typeof path !== "string") return;
    busy.value = true;
    await invoke("install_instrument", { path });
    await refreshInstruments();
    message.value = t("extInstalled");
  } catch (e) { message.value = String(e); } finally { busy.value = false; }
}
async function chooseScript() {
  try {
  const path = await open({ filters: [{ name: "Python", extensions: ["py"] }] });
  if (typeof path === "string") { script.value = path; trusted.value = false; result.value = null; }
  } catch (e) { message.value = String(e); }
}
async function run() {
  if (!trusted.value || !script.value) return;
  busy.value = true; result.value = null; message.value = "";
  source = JSON.stringify(doc.project);
  targetId = selectedTrackObject()?.id ?? "";
  try { result.value = await invoke("run_python_preset", { path: script.value, input: JSON.stringify({ apiVersion: 1, project: doc.project, selectedTrackId: targetId }) }); }
  catch (e) { message.value = String(e); }
  finally { busy.value = false; }
}
function apply() {
  if (!result.value) return;
  if (source !== JSON.stringify(doc.project)) { message.value = t("extProjectChanged"); return; }
  const preset = result.value;
  mutate(() => {
    let track = doc.project.tracks.find(track => track.id === targetId && track.type !== "audio");
    if (!track) {
      track = { id: newId(), name: preset.name, type: "instrument", instrument: "piano", color: "#D4C185", height: "normal", volume: 0.8, pan: 0, muted: false, solo: false, clips: [] };
      doc.project.tracks.push(track);
    }
    const tempo = preset.tempo ?? doc.project.tempo ?? 120;
    // Do not change existing project tempo implicitly; convert preset timing instead.
    const scale = (doc.project.tempo ?? 120) / tempo;
    const notes = preset.notes.map(note => ({ ...note, id: newId(), start: note.start * scale, duration: note.duration * scale }));
    const clip = { id: newId(), name: preset.name, start: doc.playhead, duration: Math.max(...notes.map(note => note.start + note.duration)) * 60 / (doc.project.tempo ?? 120), path: "", notes };
    track.clips.push(clip);
    doc.selectedTrack = track.id; doc.activeClipId = clip.id; doc.selectedClips = [clip.id];
  });
  result.value = null; message.value = t("extApplied");
}
onMounted(() => refreshInstruments().catch(e => message.value = String(e)));
</script>
<template>
  <div class="overlay" @keydown.stop>
    <section class="dialog" role="dialog" aria-modal="true" :aria-label="t('extensions')">
      <header><div><small>ENERGY / STUDIO</small><h2>{{ t('extensions') }}</h2></div><button :disabled="busy" @click="emit('close')">{{ t('close') }}</button></header>
      <main>
        <section><h3>{{ t('extInstruments') }}</h3><p>{{ t('extInstrumentHint') }}</p>
          <button :disabled="busy" @click="install">{{ t('extInstall') }}</button>
          <div v-for="item in instrumentPlugins" :key="item.id" class="item"><strong>{{ item.name }}</strong><code>plugin:{{ item.id }}</code><span>v{{ item.version }}</span></div>
          <p v-if="!instrumentPlugins.length">{{ t('extEmpty') }}</p>
        </section>
        <section><h3>{{ t('extPython') }}</h3><p>{{ t('extPythonHint') }}</p>
          <button :disabled="busy" @click="chooseScript">{{ t('extChoose') }}</button><code class="path">{{ script }}</code>
          <label v-if="script"><input v-model="trusted" type="checkbox" :disabled="busy" />{{ t('extTrust') }}</label>
          <button class="primary" :disabled="busy || !trusted || !script" @click="run">{{ busy ? t('extRunning') : t('extRun') }}</button>
          <div v-if="result" class="result"><strong>{{ result.name }}</strong><p>{{ t('extResult', result.notes.length) }}</p><button class="primary" @click="apply">{{ t('extApply') }}</button></div>
        </section>
        <p v-if="message" role="status" class="message">{{ message }}</p>
      </main>
    </section>
  </div>
</template>
<style scoped>
.overlay { position:fixed; inset:0; z-index:1000; display:grid; place-items:center; background:color-mix(in srgb, var(--background) 80%, transparent); }
.dialog { width:min(760px, 90vw); max-height:88vh; overflow:auto; color:var(--text); background:var(--panel); border:1px solid var(--bar); border-radius:12px; box-shadow:0 24px 80px #0008; }
header { display:flex; align-items:center; justify-content:space-between; padding:22px 28px; border-bottom:1px solid var(--bar); }
h2 { margin:6px 0 0; font-size:23px; } small { color:var(--accent); letter-spacing:2px; font-size:10px; }
main { padding:0 28px 24px; } section section { padding:18px 0; border-bottom:1px solid var(--bar); } h3 { margin:0 0 8px; font-size:15px; }
p { font-size:12px; line-height:1.65; opacity:.75; } button { border:1px solid var(--bar); border-radius:6px; background:var(--button); color:var(--button-text); padding:9px 14px; cursor:pointer; } button:disabled { opacity:.4; cursor:default; } .primary { background:var(--accent); color:var(--background); font-weight:600; }
.item { display:flex; gap:20px; align-items:center; padding:14px 0; font-size:12px; } .item code { flex:1; opacity:.6; } label { display:flex; align-items:start; gap:8px; font-size:12px; line-height:1.6; margin:16px 0; } .path { display:block; font-size:11px; overflow-wrap:anywhere; margin:12px 0; } .result { margin-top:16px; padding:16px; border:1px solid var(--accent); border-radius:8px; } .message { color:var(--accent); white-space:pre-wrap; }
</style>
