<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { t } from "../i18n";
const emit = defineEmits<{ close: []; installed: [] }>();
interface Voice { id: string; name: string; language: string; bytes: number; modelCard: string; datasetLicense: string; installed: boolean }
const voices = ref<Voice[]>([]);
const busy = ref(false);
const downloading = ref(false);
const progress = ref(0);
const message = ref("");
let unlisten: UnlistenFn | undefined;
let disposed = false;
async function refresh() { voices.value = await invoke("voice_catalog"); }
async function download(id: string) {
  busy.value = true; downloading.value = true; progress.value = 0; message.value = "";
  try { await invoke("download_voice", { id }); await refresh(); emit("installed"); message.value = t("vcInstalled"); }
  catch (e) { message.value = String(e); }
  finally { busy.value = false; downloading.value = false; }
}
async function importVoice() {
  try {
    const path = await open({ filters: [{ name: "Piper ONNX", extensions: ["onnx"] }] });
    if (typeof path !== "string") return;
    busy.value = true; message.value = "";
    await invoke("import_piper_voice", { path }); await refresh(); emit("installed"); message.value = t("vcInstalled");
  } catch (e) { message.value = String(e); } finally { busy.value = false; }
}
function cancel() { invoke("cancel_voice_download").catch(e => message.value = String(e)); }
function modelCard(url: string) { openUrl(url).catch(e => message.value = String(e)); }
onMounted(async () => {
  try {
    unlisten = await listen<number>("voice-download-progress", event => progress.value = event.payload);
    if (disposed) { unlisten(); return; }
    await refresh();
  } catch (e) { message.value = String(e); }
});
onUnmounted(() => { disposed = true; unlisten?.(); });
</script>
<template>
  <div class="overlay" @keydown.stop>
    <section class="dialog" role="dialog" aria-modal="true" :aria-label="t('vcTitle')">
      <header><div><small>ENERGY / VOICES</small><h2>{{ t('vcTitle') }}</h2></div><button :disabled="busy" @click="emit('close')">{{ t('close') }}</button></header>
      <main>
        <p>{{ t('vcHint') }}</p>
        <div class="runtime"><strong>Piper</strong><p>{{ t('vcRuntime') }}</p><code>python -m pip install piper-tts</code></div>
        <article v-for="voice in voices" :key="voice.id">
          <div class="badge">{{ voice.language }}</div><div class="details"><h3>{{ voice.name }}</h3><p>{{ Math.ceil(voice.bytes / 1048576) }} MB · ONNX · SHA-256</p><p>{{ voice.datasetLicense }}</p></div>
          <div class="actions"><button @click="modelCard(voice.modelCard)">{{ t('vcModelCard') }}</button><button class="primary" :disabled="busy || voice.installed" @click="download(voice.id)">{{ voice.installed ? t('vcPresent') : t('vcDownload') }}</button></div>
        </article>
        <div v-if="downloading" class="download"><progress :value="progress" max="1"></progress><span>{{ Math.round(progress * 100) }}%</span><button @click="cancel">{{ t('cancel') }}</button></div>
        <section class="import"><h3>{{ t('vcLocal') }}</h3><p>{{ t('vcLocalHint') }}</p><button :disabled="busy" @click="importVoice">{{ t('vcImport') }}</button></section>
        <p v-if="message" role="status" class="message">{{ message }}</p>
      </main>
    </section>
  </div>
</template>
<style scoped>
.overlay { position:fixed; inset:0; z-index:1000; display:grid; place-items:center; background:color-mix(in srgb,var(--background) 80%,transparent); }
.dialog { width:min(780px,90vw); max-height:88vh; overflow:auto; background:var(--panel); color:var(--text); border:1px solid var(--bar); border-radius:12px; box-shadow:0 24px 80px #0008; }
header { padding:24px; display:flex; align-items:center; justify-content:space-between; border-bottom:1px solid var(--bar); } small { color:var(--accent); font-size:10px; letter-spacing:2px; } h2 { margin:6px 0 0; font-size:23px; } main { padding:8px 24px 24px; } p { font-size:12px; line-height:1.6; opacity:.75; } h3 { font-size:15px; margin:0; } button { background:var(--button); color:var(--button-text); border:1px solid var(--bar); padding:9px 12px; border-radius:6px; cursor:pointer; } button:disabled { opacity:.45; cursor:default; } .primary { background:var(--accent); color:var(--background); font-weight:600; } article { display:flex; align-items:center; gap:18px; border:1px solid var(--bar); border-radius:8px; padding:18px; margin:20px 0; } .details { flex:1; } .details p { margin:6px 0; } .badge { font-size:12px; padding:14px 8px; background:var(--bar); border-radius:6px; } .actions { display:flex; flex-direction:column; gap:8px; } .runtime { border-left:2px solid var(--accent); padding:12px 16px; background:var(--background); } .runtime code { font-size:12px; } .import { border-top:1px solid var(--bar); padding-top:20px; } .download { display:flex; align-items:center; gap:12px; margin-bottom:20px; font-size:12px; } progress { flex:1; accent-color:var(--accent); } .message { color:var(--accent); white-space:pre-wrap; }
</style>
