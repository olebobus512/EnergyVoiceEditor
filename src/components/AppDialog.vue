<script lang="ts">
export interface DialogButton {
  id: string;
  label: string;
  primary?: boolean;
  danger?: boolean;
}

export type DialogRequest =
  | { kind: "message"; title: string; message: string; buttons: DialogButton[] }
  | { kind: "newProject"; defaultFolder: string }
  | { kind: "track"; name: string; volume: number; pan: number }
  | { kind: "export"; sampleRate: number; channels: number; bitDepth: number; defaultName: string }
  | { kind: "about"; version: string; engineRunning: boolean }
  | { kind: "shortcuts"; rows: { label: string; keys: string }[] };
</script>

<script setup lang="ts">
import { ref } from "vue";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { t } from "../i18n";

const props = defineProps<{ request: DialogRequest }>();
const emit = defineEmits<{ done: [value: unknown] }>();

const name = ref(props.request.kind === "newProject" ? t("newProject") : props.request.kind === "track" ? props.request.name : "");
const folder = ref(props.request.kind === "newProject" ? props.request.defaultFolder : "");
const volume = ref(props.request.kind === "track" ? props.request.volume : 1);
const pan = ref(props.request.kind === "track" ? props.request.pan : 0);
const sampleRate = ref(props.request.kind === "export" ? props.request.sampleRate : 22050);
const channels = ref(props.request.kind === "export" ? props.request.channels : 2);
const bitDepth = ref(props.request.kind === "export" ? props.request.bitDepth : 16);
const output = ref(props.request.kind === "export" ? props.request.defaultName : "");

async function chooseFolder() {
  const picked = await openDialog({ directory: true, multiple: false });
  if (typeof picked === "string") folder.value = picked;
}

async function chooseOutput() {
  const picked = await saveDialog({
    defaultPath: output.value,
    filters: [{ name: "WAV", extensions: ["wav"] }],
  });
  if (picked) output.value = picked;
}

function cancel() {
  emit("done", null);
}

function submit() {
  const request = props.request;
  if (request.kind === "newProject") {
    if (!name.value.trim() || !folder.value) return;
    emit("done", { name: name.value.trim(), folder: folder.value });
  } else if (request.kind === "track") {
    if (!name.value.trim()) return;
    emit("done", { name: name.value.trim(), volume: volume.value, pan: pan.value });
  } else if (request.kind === "export") {
    if (!output.value) return;
    emit("done", {
      sampleRate: sampleRate.value,
      channels: channels.value,
      bitDepth: bitDepth.value,
      output: output.value,
    });
  }
}

function pick(id: string) {
  emit("done", id);
}
</script>

<template>
  <div class="overlay" @pointerdown.self="cancel">
    <div class="dialog" role="dialog">
      <header class="head">
        <span>
          <template v-if="request.kind === 'message'">{{ request.title }}</template>
          <template v-else-if="request.kind === 'newProject'">{{ t("dlgNewProjectTitle") }}</template>
          <template v-else-if="request.kind === 'track'">{{ t("dlgTrackTitle") }}</template>
          <template v-else-if="request.kind === 'export'">{{ t("dlgExportTitle") }}</template>
          <template v-else-if="request.kind === 'about'">{{ t("aboutTitle") }}</template>
          <template v-else>{{ t("shortcutsTitle") }}</template>
        </span>
        <button class="close" :title="t('close')" @click="cancel">×</button>
      </header>

      <div class="body">
        <template v-if="request.kind === 'message'">
          <p>{{ request.message }}</p>
        </template>

        <template v-else-if="request.kind === 'newProject'">
          <label class="field">
            <span>{{ t("projectName") }}</span>
            <input v-model="name" autofocus />
          </label>
          <label class="field">
            <span>{{ t("projectFolder") }}</span>
            <div class="row">
              <input v-model="folder" readonly />
              <button class="btn" @click="chooseFolder">{{ t("browse") }}</button>
            </div>
          </label>
        </template>

        <template v-else-if="request.kind === 'track'">
          <label class="field">
            <span>{{ t("trackName") }}</span>
            <input v-model="name" autofocus />
          </label>
          <label class="field">
            <span>{{ t("volume") }} {{ Math.round(volume * 100) }}%</span>
            <input v-model.number="volume" type="range" min="0" max="2" step="0.01" />
          </label>
          <label class="field">
            <span>{{ t("pan") }} {{ pan.toFixed(2) }}</span>
            <input v-model.number="pan" type="range" min="-1" max="1" step="0.01" />
          </label>
        </template>

        <template v-else-if="request.kind === 'export'">
          <label class="field">
            <span>{{ t("sampleRate") }}</span>
            <select v-model.number="sampleRate">
              <option :value="22050">22050 Hz</option>
              <option :value="44100">44100 Hz</option>
              <option :value="48000">48000 Hz</option>
            </select>
          </label>
          <label class="field">
            <span>{{ t("channels") }}</span>
            <select v-model.number="channels">
              <option :value="1">{{ t("mono") }}</option>
              <option :value="2">{{ t("stereo") }}</option>
            </select>
          </label>
          <label class="field">
            <span>{{ t("bitDepth") }}</span>
            <select v-model.number="bitDepth">
              <option :value="16">16-bit</option>
              <option :value="24">24-bit</option>
            </select>
          </label>
          <label class="field">
            <span>{{ t("outputFile") }}</span>
            <div class="row">
              <input v-model="output" readonly />
              <button class="btn" @click="chooseOutput">{{ t("chooseOutput") }}</button>
            </div>
          </label>
        </template>

        <template v-else-if="request.kind === 'about'">
          <p class="app-name">Energy Voice Editor</p>
          <p>{{ t("aboutVersion", request.version) }}</p>
          <p>{{ t("aboutPublisher") }}</p>
          <p>{{ t("aboutCopyright", new Date().getFullYear()) }}</p>
          <p>{{ t("engineStatus") }}: {{ request.engineRunning ? t("engineRunning") : t("engineStopped") }}</p>
        </template>

        <template v-else>
          <table class="shortcuts">
            <tr v-for="row in request.rows" :key="row.label">
              <td>{{ row.label }}</td>
              <td class="keys">{{ row.keys }}</td>
            </tr>
          </table>
        </template>
      </div>

      <footer class="foot">
        <template v-if="request.kind === 'message'">
          <button
            v-for="button in request.buttons"
            :key="button.id"
            class="btn"
            :class="{ primary: button.primary, danger: button.danger }"
            @click="pick(button.id)"
          >{{ button.label }}</button>
        </template>
        <template v-else-if="request.kind === 'about' || request.kind === 'shortcuts'">
          <button class="btn primary" @click="cancel">{{ t("ok") }}</button>
        </template>
        <template v-else>
          <button class="btn" @click="cancel">{{ t("cancel") }}</button>
          <button class="btn primary" @click="submit">
            {{ request.kind === "export" ? t("exportAudio") : request.kind === "newProject" ? t("create") : t("ok") }}
          </button>
        </template>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(2px);
}

.dialog {
  width: min(480px, calc(100vw - 40px));
  max-height: calc(100vh - 40px);
  display: flex;
  flex-direction: column;
  background: var(--panel);
  color: var(--text);
  border: 1px solid color-mix(in srgb, var(--text) 20%, transparent);
  border-radius: 10px;
  overflow: hidden;
  box-shadow: 0 16px 44px rgba(0, 0, 0, 0.55);
  animation: dialog-in 0.14s ease-out;
  font-size: 13px;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 34px;
  padding: 0 12px;
  background: var(--bar);
  color: var(--bar-text);
  font-weight: 600;
}

.close {
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: inherit;
  font-size: 18px;
  cursor: pointer;
}

.body {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  overflow-y: auto;
}

.body p {
  margin: 0;
  line-height: 1.5;
}

.app-name {
  font-size: 16px;
  font-weight: 600;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.field > span {
  opacity: 0.75;
}

.row {
  display: flex;
  gap: 6px;
}

input,
select {
  flex: 1;
  min-width: 0;
  height: 28px;
  padding: 0 8px;
  border: 1px solid color-mix(in srgb, var(--text) 25%, transparent);
  border-radius: 4px;
  background: color-mix(in srgb, var(--panel) 90%, var(--text));
  color: var(--text);
  font: inherit;
}

input[type="range"] {
  height: 18px;
  padding: 0;
  border: none;
  background: transparent;
}

.shortcuts {
  width: 100%;
  border-collapse: collapse;
}

.shortcuts td {
  padding: 4px 6px;
  border-bottom: 1px solid color-mix(in srgb, var(--text) 12%, transparent);
}

.shortcuts .keys {
  text-align: right;
  font-family: Consolas, "Courier New", monospace;
  white-space: nowrap;
}

.foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 10px 16px;
  border-top: 1px solid color-mix(in srgb, var(--text) 12%, transparent);
}

.btn {
  height: 28px;
  padding: 0 14px;
  border: 2px solid transparent;
  border-radius: 4px;
  background: var(--button);
  color: var(--button-text);
  font: inherit;
  cursor: pointer;
  white-space: nowrap;
}

.btn.primary {
  border-color: var(--accent);
}

.btn.danger {
  background: #b3261e;
  color: #fff;
}

@keyframes dialog-in {
  from {
    opacity: 0;
    transform: scale(0.97);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}
</style>
