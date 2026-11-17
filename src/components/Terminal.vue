<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { clearLog, log, logError, logInfo, logStore, type LogEntry, type LogLevel } from "../logger";
import { showContextMenu } from "../contextmenu";
import PtyTerminal from "./PtyTerminal.vue";
const terminalTab = ref(false);
const pty = ref<InstanceType<typeof PtyTerminal>>();
import { t } from "../i18n";

const emit = defineEmits<{
  hide: [];
}>();

interface ShellLine {
  stream: "stdout" | "stderr";
  text: string;
}

const MAX_HISTORY = 200;

const enabledLevels = ref<Record<LogLevel, boolean>>({ INFO: true, WARNING: true, ERROR: true, DEBUG: false });
const search = ref("");
const paused = ref(false);
const autoscroll = ref(true);
const frozen = ref<LogEntry[]>([]);
const body = ref<HTMLElement | null>(null);
const input = ref("");
const history = ref<string[]>([]);
const historyIndex = ref(-1);
let commandId = 0;
const unlisteners: UnlistenFn[] = [];

const live = computed<LogEntry[]>(() => {
  const query = search.value.trim().toLowerCase();
  return logStore.entries.filter(
    (entry) =>
      enabledLevels.value[entry.level] &&
      (query === "" || entry.message.toLowerCase().includes(query) || entry.source.toLowerCase().includes(query)),
  );
});

const visible = computed(() => (paused.value ? frozen.value : live.value));

function togglePause() {
  if (!paused.value) frozen.value = [...live.value];
  paused.value = !paused.value;
}

function copyOutput() {
  if (terminalTab.value) { pty.value?.copy().catch(e => logError(String(e))); return; }
  const selection = window.getSelection()?.toString() ?? "";
  const text = selection || visible.value.map(formatEntry).join("\n");
  void navigator.clipboard.writeText(text);
}

function copyText(text: string) {
  navigator.clipboard.writeText(text).catch((error) => logError(t("logError", String(error)), "app"));
}

function selectAllOutput() {
  if (!body.value) return;
  const range = document.createRange();
  range.selectNodeContents(body.value);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
}

/** Right-click menu of the output area. The selection is read now, because opening the menu must not depend on it later. */
function onOutputContext(event: MouseEvent) {
  const selected = window.getSelection()?.toString() ?? "";
  showContextMenu(event.clientX, event.clientY, [
    { label: t("copy"), disabled: !selected, action: () => copyText(selected) },
    { label: t("copyAllOutput"), disabled: visible.value.length === 0, action: () => copyText(visible.value.map(formatEntry).join("\n")) },
    { label: t("selectAll"), disabled: visible.value.length === 0, action: selectAllOutput },
    { separator: true },
    { label: t("clear"), danger: true, action: clear },
    { separator: true },
    { label: paused.value ? t("resume") : t("pause"), action: togglePause },
    { label: t("autoscroll"), checked: autoscroll.value, action: () => (autoscroll.value = !autoscroll.value) },
    { label: t("restartShell"), action: restart },
  ]);
}

function formatEntry(entry: LogEntry): string {
  return `${entry.time} [${entry.level}] ${entry.source}: ${entry.message}`;
}

async function submit() {
  const command = input.value.trim();
  if (!command) return;
  history.value = [command, ...history.value.filter((item) => item !== command)].slice(0, MAX_HISTORY);
  historyIndex.value = -1;
  input.value = "";
  logInfo(`PS> ${command}`, "input");
  try {
    commandId += 1;
    await invoke("shell_run", { id: commandId, command });
  } catch (error) {
    logError(t("shellError", String(error)), "ps");
  }
}

function recall(direction: -1 | 1) {
  if (history.value.length === 0) return;
  historyIndex.value = Math.min(history.value.length - 1, Math.max(-1, historyIndex.value + direction));
  input.value = historyIndex.value >= 0 ? history.value[historyIndex.value] : "";
}

async function restart() {
  try {
    await invoke("shell_restart");
    logInfo("PowerShell restarted", "ps");
  } catch (error) {
    logError(t("shellError", String(error)), "ps");
  }
}

function clear() {
  if (terminalTab.value) { pty.value?.clear(); return; }
  clearLog();
  frozen.value = [];
}

watch(
  () => visible.value.length,
  () =>
    nextTick(() => {
      if (autoscroll.value && !paused.value && body.value) body.value.scrollTop = body.value.scrollHeight;
    }),
);

onMounted(async () => {
  unlisteners.push(
    await listen<ShellLine>("shell-output", (event) => {
      const level: LogLevel = event.payload.stream === "stderr" ? "ERROR" : "INFO";
      if (event.payload.text) log(level, "ps", event.payload.text);
    }),
  );
});

onUnmounted(() => unlisteners.forEach((unlisten) => unlisten()));
</script>

<template>
  <section class="terminal">
    <header class="term-header">
      <button class="btn-action" @click="terminalTab = !terminalTab">{{ terminalTab ? "OUTPUT" : "TERMINAL" }}</button>
      <div class="header-actions">
        <button class="btn-action" title="Clear console" @click="clear">{{ t('clear') }}</button>
        <button class="btn-action" title="Copy console output" @click="copyOutput">{{ t('copy') }}</button>
        <button class="btn-action" title="Hide console" @click="emit('hide')">{{ t('hide') }}</button>
      </div>
    </header>

    <PtyTerminal ref="pty" :active="terminalTab" v-show="terminalTab" style="flex:1" />
    <div v-show="!terminalTab" ref="body" class="term-body" @contextmenu.prevent="onOutputContext">
      <div v-for="entry in visible" :key="entry.id" class="line" :class="entry.level.toLowerCase()">
        <span class="prompt-glyph">›</span>
        <span class="time">{{ entry.time }}</span>
        <span class="src">[{{ entry.source }}]</span>
        <span class="msg">{{ entry.message }}</span>
      </div>
      <div v-if="visible.length === 0" class="empty-hint">
        <span class="prompt-glyph">›</span> Ready. Energy Voice Editor console.
      </div>
    </div>

    <div v-show="!terminalTab" class="prompt">
      <span class="ps">›</span>
      <input
        v-model="input"
        class="cmd"
        :placeholder="t('terminalInput')"
        spellcheck="false"
        @keydown.enter.prevent="submit"
        @keydown.up.prevent="recall(1)"
        @keydown.down.prevent="recall(-1)"
      />
    </div>
  </section>
</template>

<style scoped>
.terminal {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--panel);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  color: var(--text);
  font-family: "JetBrains Mono", Consolas, monospace;
  font-size: 12px;
  overflow: hidden;
  box-sizing: border-box;
}

.term-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 40px;
  flex: none;
  padding: 0 14px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  background: transparent;
}

.term-title {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.8px;
  color: color-mix(in srgb, var(--text) 60%, transparent);
  text-transform: uppercase;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.btn-action {
  height: 24px;
  padding: 0 10px;
  border: none;
  border-radius: 6px;
  background: var(--button);
  color: color-mix(in srgb, var(--text) 60%, transparent);
  font-size: 11px;
  font-family: inherit;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-action:hover {
  background: var(--bar);
  color: var(--accent);
}

.term-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 10px 12px;
  font-size: 12px;
  line-height: 1.6;
}

.line {
  display: flex;
  align-items: baseline;
  gap: 8px;
  white-space: pre-wrap;
  word-break: break-word;
}

.prompt-glyph {
  color: var(--accent);
  font-weight: bold;
}

.time {
  color: #555e75;
  font-size: 11px;
}

.src {
  color: #717d9a;
  font-size: 11px;
}

.msg {
  color: var(--text);
}

.line.error .msg {
  color: #ff5c7a;
}

.line.warning .msg {
  color: var(--accent);
}

.empty-hint {
  color: color-mix(in srgb, var(--text) 60%, transparent);
  font-style: italic;
}

.prompt {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: none;
  height: 36px;
  padding: 0 12px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  background: rgba(0, 0, 0, 0.15);
}

.ps {
  color: var(--accent);
  font-size: 15px;
  font-weight: 800;
}

.cmd {
  flex: 1;
  min-width: 0;
  height: 26px;
  padding: 0 4px;
  border: none;
  background: transparent;
  color: var(--text);
  font-family: inherit;
  font-size: 12px;
  outline: none;
}

.cmd::placeholder {
  color: #555e75;
}
</style>
