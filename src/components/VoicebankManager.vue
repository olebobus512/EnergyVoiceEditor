<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { logError, logInfo, logWarning } from "../logger";
import { t, type MessageKey } from "../i18n";

type Status = "ready" | "missing_dependencies" | "incompatible" | "corrupted";

interface BankInfo {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  languages: string[];
  synthesisTypes: string[];
  supportedBackends: string[];
  usableBackends: string[];
  license: { name: string; url?: string; commercialUse?: boolean } | null;
  tags: string[];
  status: Status;
  problems: string[];
  sizeBytes: number;
  path: string;
  hasPreview: boolean;
}

interface InstallOutcome {
  id: string;
  name: string;
  version: string;
  replaced: string | null;
  warnings: string[];
}

interface CommandError {
  code: string;
  message: string;
  installed?: string;
  incoming?: string;
}

const emit = defineEmits<{ close: [] }>();

const banks = ref<BankInfo[]>([]);
const selectedId = ref("");
const search = ref("");
const filter = ref<"all" | "ready" | "problems">("all");
const busy = ref("");
const message = ref<{ kind: "info" | "error"; text: string } | null>(null);
/** A question waiting for an answer inside the dialog: replace an installed bank, or uninstall one. */
const confirm = ref<{ text: string; yes: string; action: () => Promise<void> } | null>(null);
const previewing = ref(false);
let previewAudio: HTMLAudioElement | null = null;
let previewUrl = "";

const STATUS_LABELS: Record<Status, MessageKey> = {
  ready: "vbStatusReady",
  missing_dependencies: "vbStatusMissingDeps",
  incompatible: "vbStatusIncompatible",
  corrupted: "vbStatusCorrupted",
};

const visible = computed(() => {
  const query = search.value.trim().toLowerCase();
  return banks.value.filter((bank) => {
    if (filter.value === "ready" && bank.status !== "ready") return false;
    if (filter.value === "problems" && bank.status === "ready") return false;
    if (!query) return true;
    return [bank.name, bank.id, bank.author, bank.description, ...bank.languages, ...bank.tags].join(" ").toLowerCase().includes(query);
  });
});

const selected = computed(() => banks.value.find((bank) => bank.id === selectedId.value) ?? null);

function errorText(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) return String((error as CommandError).message);
  return String(error);
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function report(kind: "info" | "error", text: string) {
  message.value = { kind, text };
  if (kind === "error") logError(text);
  else logInfo(text);
}

async function refresh() {
  try {
    banks.value = await invoke<BankInfo[]>("evbank_list");
    if (!banks.value.some((bank) => bank.id === selectedId.value)) selectedId.value = banks.value[0]?.id ?? "";
  } catch (error) {
    report("error", errorText(error));
  }
}

async function run(label: string, work: () => Promise<void>) {
  if (busy.value) return;
  busy.value = label;
  message.value = null;
  confirm.value = null;
  try {
    await work();
  } finally {
    busy.value = "";
  }
}

async function install(path: string, kind: "archive" | "folder", replace: boolean) {
  try {
    const outcome = await invoke<InstallOutcome>("evbank_install", { path, kind, replace });
    report("info", outcome.replaced ? t("vbReplaced", outcome.name, outcome.replaced, outcome.version) : t("vbInstalled", outcome.name, outcome.version));
    for (const warning of outcome.warnings) logWarning(`${outcome.name}: ${warning}`);
    await refresh();
    selectedId.value = outcome.id;
  } catch (error) {
    const failure = error as CommandError;
    if (failure && failure.code === "already_installed") {
      confirm.value = {
        text: t("vbConfirmReplace", failure.message.match(/'([^']+)'/)?.[1] ?? "", failure.installed ?? "?", failure.incoming ?? "?"),
        yes: t("vbReplace"),
        action: () => run(t("vbWorking"), () => install(path, kind, true)),
      };
      return;
    }
    report("error", errorText(error));
  }
}

async function importArchive() {
  const picked = await openDialog({ multiple: false, filters: [{ name: "Energy Voice Bank", extensions: ["evbank"] }] });
  if (typeof picked === "string") await run(t("vbWorking"), () => install(picked, "archive", false));
}

async function importFolder() {
  const picked = await openDialog({ multiple: false, directory: true });
  if (typeof picked === "string") await run(t("vbWorking"), () => install(picked, "folder", false));
}

async function packFolder() {
  const folder = await openDialog({ multiple: false, directory: true });
  if (typeof folder !== "string") return;
  const destination = await saveDialog({ filters: [{ name: "Energy Voice Bank", extensions: ["evbank"] }], defaultPath: "voicebank.evbank" });
  if (!destination) return;
  await run(t("vbWorking"), async () => {
    try {
      const id = await invoke<string>("evbank_pack", { folder, destination });
      report("info", t("vbPacked", id, destination));
    } catch (error) {
      report("error", errorText(error));
    }
  });
}

async function verify(bank: BankInfo) {
  await run(t("vbVerifying"), async () => {
    try {
      const result = await invoke<{ fileCount: number; totalBytes: number; warnings: string[] }>("evbank_verify", { id: bank.id });
      report("info", t("vbVerified", bank.name, result.fileCount, formatSize(result.totalBytes)));
      for (const warning of result.warnings) logWarning(`${bank.name}: ${warning}`);
    } catch (error) {
      report("error", errorText(error));
    }
    await refresh();
  });
}

function askUninstall(bank: BankInfo) {
  confirm.value = {
    text: t("vbConfirmUninstall", bank.name),
    yes: t("vbUninstall"),
    action: () =>
      run(t("vbWorking"), async () => {
        stopPreview();
        try {
          await invoke("evbank_uninstall", { id: bank.id });
          report("info", t("vbUninstalled", bank.name));
        } catch (error) {
          report("error", errorText(error));
        }
        await refresh();
      }),
  };
}

async function exportBank(bank: BankInfo) {
  const destination = await saveDialog({ filters: [{ name: "Energy Voice Bank", extensions: ["evbank"] }], defaultPath: `${bank.id}.evbank` });
  if (!destination) return;
  await run(t("vbWorking"), async () => {
    try {
      await invoke("evbank_export", { id: bank.id, destination });
      report("info", t("vbExported", destination));
    } catch (error) {
      report("error", errorText(error));
    }
  });
}

function stopPreview() {
  previewAudio?.pause();
  previewAudio = null;
  if (previewUrl) URL.revokeObjectURL(previewUrl);
  previewUrl = "";
  previewing.value = false;
}

async function togglePreview(bank: BankInfo) {
  if (previewing.value) return stopPreview();
  try {
    const data = await invoke<ArrayBuffer>("evbank_preview", { id: bank.id });
    stopPreview();
    previewUrl = URL.createObjectURL(new Blob([data], { type: "audio/wav" }));
    previewAudio = new Audio(previewUrl);
    previewAudio.onended = stopPreview;
    await previewAudio.play();
    previewing.value = true;
  } catch (error) {
    stopPreview();
    report("error", errorText(error));
  }
}

async function openFolder() {
  try {
    await revealItemInDir(await invoke<string>("evbank_dir"));
  } catch (error) {
    report("error", errorText(error));
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && !busy.value) {
    event.stopPropagation();
    if (confirm.value) confirm.value = null;
    else emit("close");
  }
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown, true);
  void refresh();
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown, true);
  stopPreview();
});
</script>

<template>
  <div class="overlay" @pointerdown.self="!busy && emit('close')">
    <div class="dialog" role="dialog" :aria-label="t('voicebankManager')">
      <div class="dialog-header">
        <span>{{ t("voicebankManager") }}</span>
        <button class="btn small" :title="t('close')" :disabled="!!busy" @click="emit('close')">×</button>
      </div>

      <div class="toolbar">
        <button class="btn" :disabled="!!busy" @click="importArchive">{{ t("vbImportArchive") }}</button>
        <button class="btn" :disabled="!!busy" @click="importFolder">{{ t("vbImportFolder") }}</button>
        <button class="btn" :disabled="!!busy" @click="packFolder">{{ t("vbPack") }}</button>
        <span class="grow"></span>
        <button class="btn" @click="openFolder">{{ t("vbOpenFolder") }}</button>
        <button class="btn" :disabled="!!busy" @click="refresh">{{ t("vbRefresh") }}</button>
      </div>

      <div v-if="busy" class="note busy">{{ busy }}</div>
      <div v-else-if="confirm" class="note confirm">
        <span>{{ confirm.text }}</span>
        <span class="grow"></span>
        <button class="btn" @click="confirm.action()">{{ confirm.yes }}</button>
        <button class="btn" @click="confirm = null">{{ t("cancel") }}</button>
      </div>
      <div v-else-if="message" class="note" :class="message.kind">{{ message.text }}</div>

      <div class="body">
        <div class="list-pane">
          <div class="filters">
            <input v-model="search" type="search" :placeholder="t('vbSearch')" />
            <select v-model="filter">
              <option value="all">{{ t("vbFilterAll") }}</option>
              <option value="ready">{{ t("vbFilterReady") }}</option>
              <option value="problems">{{ t("vbFilterProblems") }}</option>
            </select>
          </div>
          <div class="list">
            <button
              v-for="bank in visible"
              :key="bank.id"
              class="card"
              :class="{ selected: bank.id === selectedId }"
              @click="selectedId = bank.id"
            >
              <span class="card-title">{{ bank.name }}</span>
              <span class="badge" :class="bank.status">{{ t(STATUS_LABELS[bank.status]) }}</span>
              <span class="card-sub">{{ bank.version }} · {{ bank.author || bank.id }}</span>
            </button>
            <p v-if="banks.length === 0" class="empty">{{ t("vbEmpty") }}</p>
            <p v-else-if="visible.length === 0" class="empty">{{ t("vbNoMatch") }}</p>
          </div>
        </div>

        <div class="detail-pane">
          <template v-if="selected">
            <h2>{{ selected.name }} <small>{{ selected.version }}</small></h2>
            <p v-if="selected.description" class="desc">{{ selected.description }}</p>

            <dl>
              <dt>{{ t("vbAuthor") }}</dt>
              <dd>{{ selected.author || "—" }}</dd>
              <dt>{{ t("vbLanguages") }}</dt>
              <dd>{{ selected.languages.join(", ") || "—" }}</dd>
              <dt>{{ t("vbTypes") }}</dt>
              <dd>{{ selected.synthesisTypes.join(", ") || "—" }}</dd>
              <dt>{{ t("vbBackends") }}</dt>
              <dd>{{ selected.supportedBackends.join(", ") || "—" }}</dd>
              <dt>{{ t("vbLicense") }}</dt>
              <dd>
                {{ selected.license?.name ?? "—" }}
                <span v-if="selected.license?.commercialUse === false"> ({{ t("vbNoCommercial") }})</span>
              </dd>
              <dt>{{ t("vbSize") }}</dt>
              <dd>{{ formatSize(selected.sizeBytes) }}</dd>
              <dt>{{ t("vbLocation") }}</dt>
              <dd class="path">{{ selected.path }}</dd>
            </dl>

            <p v-if="selected.status === 'ready' && selected.usableBackends.length === 0" class="notice">{{ t("vbNoBackend") }}</p>
            <div v-if="selected.problems.length" class="problems">
              <strong>{{ t("vbProblems") }}</strong>
              <ul>
                <li v-for="problem in selected.problems" :key="problem">{{ problem }}</li>
              </ul>
            </div>

            <div class="actions">
              <button class="btn" :disabled="!!busy" @click="verify(selected)">{{ t("vbVerify") }}</button>
              <button v-if="selected.hasPreview" class="btn" :disabled="!!busy" @click="togglePreview(selected)">
                {{ previewing ? t("vbStopPreview") : t("vbPreview") }}
              </button>
              <button class="btn" :disabled="!!busy || selected.status === 'incompatible'" @click="exportBank(selected)">{{ t("vbExport") }}</button>
              <span class="grow"></span>
              <button class="btn danger" :disabled="!!busy" @click="askUninstall(selected)">{{ t("vbUninstall") }}</button>
            </div>
          </template>
          <p v-else class="empty">{{ t("vbSelectOne") }}</p>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  z-index: 40;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.55);
}

.dialog {
  width: min(980px, 94vw);
  height: min(640px, 90vh);
  display: flex;
  flex-direction: column;
  background: var(--panel);
  color: var(--text);
  border: 1px solid var(--accent);
  border-radius: 10px;
  overflow: hidden;
  box-shadow: 0 16px 44px rgba(0, 0, 0, 0.55);
  font-size: 13px;
}

.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--bar);
  color: var(--bar-text);
  font-size: 16px;
  font-weight: 600;
}

.toolbar,
.actions,
.filters {
  display: flex;
  align-items: center;
  gap: 8px;
}

.toolbar {
  padding: 10px 14px;
  border-bottom: 1px solid color-mix(in srgb, var(--text) 14%, transparent);
}

.grow {
  flex: 1;
}

.btn {
  height: 28px;
  padding: 0 12px;
  border: 2px solid transparent;
  border-radius: 5px;
  background: var(--button);
  color: var(--button-text);
  font: inherit;
  font-weight: 500;
  white-space: nowrap;
  cursor: pointer;
}

.btn.small {
  height: 24px;
  padding: 0 8px;
}

.btn:disabled {
  opacity: 0.45;
  cursor: default;
}

.btn:hover:not(:disabled) {
  filter: brightness(1.08);
}

.btn.danger:not(:disabled) {
  background: #8f2d2d;
  color: #fff;
}

.note {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  background: color-mix(in srgb, var(--text) 8%, transparent);
  white-space: pre-wrap;
}

.note.error {
  background: color-mix(in srgb, #d93a3a 24%, transparent);
}

.note.busy {
  opacity: 0.8;
}

.body {
  flex: 1;
  display: flex;
  min-height: 0;
}

.list-pane {
  width: 300px;
  display: flex;
  flex-direction: column;
  border-right: 1px solid color-mix(in srgb, var(--text) 14%, transparent);
}

.filters {
  padding: 10px;
}

.filters input {
  flex: 1;
  min-width: 0;
}

.filters input[type="search"],
.filters select {
  max-width: none;
  height: 26px;
  padding: 0 8px;
  border: 1px solid color-mix(in srgb, var(--text) 30%, transparent);
  border-radius: 5px;
  background: color-mix(in srgb, var(--panel) 85%, var(--text));
  color: var(--text);
  font: inherit;
}

.filters select {
  flex: none;
  width: 110px;
}

.list {
  flex: 1;
  overflow-y: auto;
  padding: 0 10px 10px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.card {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 2px 8px;
  padding: 8px 10px;
  border: 1px solid color-mix(in srgb, var(--text) 16%, transparent);
  border-radius: 7px;
  background: color-mix(in srgb, var(--panel) 92%, var(--text));
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.card:hover {
  background: color-mix(in srgb, var(--panel) 84%, var(--text));
}

.card.selected {
  border-color: var(--accent);
}

.card-title {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-sub {
  grid-column: 1 / -1;
  font-size: 11.5px;
  opacity: 0.65;
}

.badge {
  align-self: center;
  padding: 1px 7px;
  border-radius: 9px;
  font-size: 10.5px;
  font-weight: 600;
  background: #2f7d4a;
  color: #fff;
}

.badge.missing_dependencies {
  background: #a8772a;
}

.badge.incompatible,
.badge.corrupted {
  background: #a83a3a;
}

.detail-pane {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 14px 18px;
}

h2 {
  margin: 0 0 6px;
  font-size: 18px;
}

h2 small {
  font-size: 12px;
  font-weight: 400;
  opacity: 0.6;
}

.desc {
  margin: 0 0 12px;
  opacity: 0.85;
}

dl {
  display: grid;
  grid-template-columns: 110px 1fr;
  gap: 5px 12px;
  margin: 0 0 12px;
}

dt {
  opacity: 0.6;
}

dd {
  margin: 0;
  min-width: 0;
}

.path {
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 11.5px;
  word-break: break-all;
}

.notice {
  padding: 8px 10px;
  border-left: 3px solid #d9a441;
  background: color-mix(in srgb, #d9a441 14%, transparent);
}

.problems {
  padding: 8px 10px;
  border-left: 3px solid #d93a3a;
  background: color-mix(in srgb, #d93a3a 14%, transparent);
}

.problems ul {
  margin: 4px 0 0;
  padding-left: 18px;
}

.actions {
  margin-top: 14px;
}

.empty {
  margin: 16px 4px;
  opacity: 0.6;
}
</style>
