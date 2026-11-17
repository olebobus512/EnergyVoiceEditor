<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { doc, mutate, newId, selectedTrackObject } from "../project";
import { t } from "../i18n";
const emit = defineEmits<{ close: [] }>();
interface Voice { id: string; name: string; engine: string; languages: string[]; supportsPitch: boolean }
interface Take { path: string; durationSec: number; sampleRate: number }
const voices = ref<Voice[]>([]);
const voiceId = ref("");
const text = ref("");
const speed = ref(1);
const pitch = ref(0);
const busy = ref(false);
const message = ref("");
const progress = ref(0);
const take = ref<Take>();
const selected = computed(() => voices.value.find(v => v.id === voiceId.value));
let takeVoice = "";
let takeText = "";
let preview: HTMLAudioElement | undefined;
let url = "";
let unlisten: UnlistenFn | undefined;
let disposed = false;
async function refresh() {
  busy.value = true;
  try {
    voices.value = (await invoke<Voice[]>("list_voices", { refresh:true })).filter(v => v.engine !== "debug");
    if (!voices.value.some(v => v.id === voiceId.value)) voiceId.value = voices.value[0]?.id ?? "";
  } catch (e) { message.value = String(e); } finally { busy.value = false; }
}
function stop() { preview?.pause(); preview = undefined; if (url) URL.revokeObjectURL(url); url = ""; }
async function generate() {
  if (!selected.value || !text.value.trim()) return;
  busy.value = true; take.value = undefined; progress.value = 0; message.value = ""; stop();
  const voice = selected.value;
  const input = text.value.trim();
  try {
    const result = await invoke<Take>("synthesize", { text:input, voice:voice.id, speed:speed.value, pitch:voice.supportsPitch ? pitch.value : 0 });
    if (!disposed) { take.value = result; takeVoice = voice.id; takeText = input; }
  } catch (e) { message.value = String(e); } finally { busy.value = false; }
}
async function play() {
  if (!take.value) return;
  stop();
  try {
    const data = await invoke<ArrayBuffer>("read_audio", { path:take.value.path });
    if (disposed) return;
    url = URL.createObjectURL(new Blob([data], {type:"audio/wav"})); preview = new Audio(url); preview.volume = doc.masterVolume;
    await preview.play();
  } catch (e) { message.value = String(e); }
}
function apply() {
  if (!take.value) return;
  const result = take.value;
  mutate(() => {
    let track = selectedTrackObject();
    if (!track || track.type !== "voice") {
      track = {id:newId(),name:t("voice"),type:"voice",voiceId:takeVoice,color:"#D4C185",height:"normal",volume:0.8,pan:0,muted:false,solo:false,clips:[]};
      doc.project.tracks.push(track);
    }
    const clip = { id:newId(),name:takeText.slice(0,32),text:takeText,path:result.path,start:doc.playhead,duration:result.durationSec };
    track.clips.push(clip); track.voiceId = takeVoice; doc.selectedTrack = track.id; doc.selectedClips = [clip.id];
  });
  take.value = undefined; message.value = t("speechAdded"); stop();
}
onMounted(async () => {
  unlisten = await listen<number>("synthesis-progress", e => progress.value = e.payload);
  if (disposed) { unlisten(); return; }
  await refresh();
});
onUnmounted(() => { disposed = true; unlisten?.(); stop(); });
</script>
<template>
  <div class="overlay" @keydown.stop><section class="dialog" role="dialog" aria-modal="true" :aria-label="t('speechTitle')">
    <header><div><small>ENERGY / VOICE STUDIO</small><h2>{{ t('speechTitle') }}</h2></div><button :disabled="busy" @click="emit('close')">{{ t('close') }}</button></header>
    <main>
      <div class="row"><label>{{ t('voice') }}<select v-model="voiceId" :disabled="busy"><option v-for="voice in voices" :key="voice.id" :value="voice.id">{{ voice.name }} · {{ voice.engine }}</option></select></label><button :disabled="busy" @click="refresh">{{ t('vbRefresh') }}</button></div>
      <p v-if="!voices.length && !busy">{{ t('speechNoVoices') }}</p>
      <textarea v-model="text" :disabled="busy" maxlength="2000" :placeholder="t('textPlaceholder')" :aria-label="t('textPlaceholder')"></textarea>
      <div class="count">{{ text.length }} / 2000</div>
      <div class="row"><label>{{ t('speed') }} · {{ speed }}×<input v-model.number="speed" :disabled="busy" type="range" min="0.5" max="2" step="0.05" /></label><label>{{ t('pitch') }} · {{ pitch }}<input v-model.number="pitch" :disabled="busy || !selected?.supportsPitch" type="range" min="-12" max="12" step="1" /></label></div>
      <progress v-if="busy" :value="progress" max="1"></progress>
      <div class="row actions"><button class="primary" :disabled="busy || !selected || !text.trim()" @click="generate">{{ t('generate') }}</button><template v-if="take"><span>{{ take.durationSec.toFixed(2) }} s</span><button @click="play">{{ t('speechPreview') }}</button><button @click="stop">{{ t('stopTitle') }}</button><button class="primary" @click="apply">{{ t('speechAdd') }}</button></template></div>
      <p v-if="message" role="status" class="message">{{ message }}</p>
    </main>
  </section></div>
</template>
<style scoped>
.overlay {position:fixed;inset:0;z-index:1000;display:grid;place-items:center;background:color-mix(in srgb,var(--background) 80%,transparent)}
.dialog {width:min(760px,90vw);max-height:88vh;overflow:auto;background:var(--panel);color:var(--text);border:1px solid var(--bar);border-radius:12px;box-shadow:0 24px 80px #0008}
header {display:flex;align-items:center;justify-content:space-between;padding:24px;border-bottom:1px solid var(--bar)} small {font-size:10px;letter-spacing:2px;color:var(--accent)} h2 {margin:6px 0 0;font-size:23px} main {padding:24px} .row {display:flex;gap:16px;align-items:center} label {flex:1;display:flex;flex-direction:column;gap:10px;font-size:12px} button,select {background:var(--button);color:var(--button-text);border:1px solid var(--bar);border-radius:6px;padding:10px 12px} button {cursor:pointer} button:disabled {opacity:.4;cursor:default} .primary {background:var(--accent);color:var(--background);font-weight:600} textarea {box-sizing:border-box;width:100%;height:170px;resize:vertical;margin-top:20px;border:1px solid var(--bar);background:var(--background);color:var(--text);border-radius:8px;padding:14px;font:14px/1.7 inherit} .count {font-size:10px;text-align:right;opacity:.6;margin:8px 0 20px} input,progress {accent-color:var(--accent)} progress {width:100%;margin-top:20px} .actions {margin-top:24px;font-size:12px;flex-wrap:wrap} p {font-size:12px;line-height:1.6} .message {color:var(--accent);white-space:pre-wrap}
</style>
