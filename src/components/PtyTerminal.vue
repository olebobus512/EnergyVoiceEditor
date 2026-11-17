<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch, nextTick } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { t } from "../i18n";
const props = defineProps<{ active: boolean }>();
const host = ref<HTMLElement>();
const error = ref("");
const terminal = new Terminal({ cursorBlink: true, fontSize: 12, fontFamily: "Consolas, monospace", scrollback: 5000 });
const fit = new FitAddon();
let unlisten: UnlistenFn | undefined;
let observer: ResizeObserver | undefined;
let disposed = false;
let started = false;
let ready = false;
let writes = Promise.resolve();
watch(() => props.active, async active => {
  if (!active || !ready) return;
  await nextTick(); fit.fit();
  if (!started) await restart();
  terminal.focus();
});
defineExpose({ clear: () => terminal.clear(), copy: () => navigator.clipboard.writeText(terminal.getSelection()) });
async function restart() {
  try { fit.fit(); terminal.clear(); await invoke("pty_start", { cols: terminal.cols, rows: terminal.rows }); error.value = ""; started = true; terminal.focus(); }
  catch (e) { error.value = String(e); }
}
onMounted(async () => {
  terminal.loadAddon(fit);
  terminal.open(host.value!);
  fit.fit();
  terminal.onData(data => { writes = writes.then(() => invoke<void>("pty_write", { data })).catch(e => { error.value = String(e); }); });
  unlisten = await listen<number[]>("pty-data", event => terminal.write(new Uint8Array(event.payload)));
  if (disposed) { unlisten(); return; }
  observer = new ResizeObserver(() => { fit.fit(); invoke("pty_resize", { cols: terminal.cols, rows: terminal.rows }).catch(e => error.value = String(e)); });
  observer.observe(host.value!);
  ready = true;
  if (props.active) await restart();
});
onUnmounted(() => { disposed = true; observer?.disconnect(); unlisten?.(); terminal.dispose(); });
</script>
<template>
  <section class="pty" @keydown.stop @keyup.stop @contextmenu.stop>
    <header><span>PowerShell</span><button @click="restart">{{ t('restartShell') }}</button></header>
    <div v-if="error" class="error">{{ error }}</div>
    <div ref="host" class="screen"></div>
  </section>
</template>
<style scoped>
.pty { height: 100%; display: flex; flex-direction: column; background: var(--background); color: var(--text); min-height: 0; }
header { display:flex; align-items:center; justify-content:space-between; padding:8px 12px; font-size:12px; }
button { background:var(--button); color:var(--button-text); border:1px solid var(--bar); border-radius:5px; padding:5px 10px; cursor:pointer; }
.screen { flex:1; min-height:0; padding:4px; overflow:hidden; }
.error { color:var(--accent); font-size:12px; padding:8px; }
</style>
