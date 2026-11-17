<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch, watchEffect } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import Arrangement from "./components/Arrangement.vue";
import AppDialog, { type DialogRequest } from "./components/AppDialog.vue";
import ContextMenu from "./components/ContextMenu.vue";
import MenuBar, { type MenuDef, type MenuItem } from "./components/MenuBar.vue";
import PianoRoll from "./components/PianoRoll.vue";
import SpeechStudio from "./components/SpeechStudio.vue";
import VoiceCatalog from "./components/VoiceCatalog.vue";
import ExtensionsDialog from "./components/ExtensionsDialog.vue";
import { refreshInstruments } from "./extensions";
import SettingsDialog from "./components/SettingsDialog.vue";
import VoicebankManager from "./components/VoicebankManager.vue";
import Splitter from "./components/Splitter.vue";
import Terminal from "./components/Terminal.vue";
import { type ArrangementContext, hideContextMenu, showContextMenu } from "./contextmenu";
import { t } from "./i18n";
import {
  playMetronomeClick,
  setMasterVolume,
} from "./instruments";
import {
  acceptsClips,
  addClip,
  addTrack,
  allClips,
  canRedo,
  canUndo,
  clearSolo,
  clearTrack,
  copySelectedClips,
  cutSelectedClips,
  deleteSelectedClips,
  deleteTrack,
  doc,
  duplicateSelectedClips,
  duplicateTrack,
  emptyProject,
  mutate,
  moveTrack,
  parseProject,
  pasteClips,
  redo,
  resetProject,
  selectAllClips,
  selectedTrackObject,
  serializeProject,
  setTrackColor,
  setTrackHeight,
  setTrackVoice,
  timelineEnd,
  toggleTrackFlag,
  undo,
  markSaved,
  type Track,
  type TrackHeight,
  type TrackType,
} from "./project";
import { TRACK_PALETTE } from "./trackColors";
import { logError, logInfo, logWarning } from "./logger";
import { DEFAULT_WORKSPACE, applySettings, cloneSettings, loadSettings, persistSettings, settings } from "./settings";
import { SHORTCUTS, dispatchShortcut, isTextInput, type ActionId } from "./shortcuts";

interface Voice {
  id: string;
  name: string;
  languages: string[];
  engine: string;
  gender: string;
  supportsPitch: boolean;
}

interface AudioInfo {
  path: string;
  durationSec: number;
  sampleRate: number;
  channels: number;
}

interface RenderResult {
  path: string;
  durationSec: number;
}

interface AppInfo {
  version: string;
  engineRunning: boolean;
}

const ws = settings.workspace;
const voices = ref<Voice[]>([]);
const selectedVoice = ref("");
const progress = ref(0);
const tool = ref<"select" | "draw" | "erase">("select");
const snap = ref(true);
const gridSec = ref(0.5);
const showSettings = ref(false);
const showExtensions = ref(false);
const showVoiceCatalog = ref(false);
const showSpeech = ref(false);
const showVoicebanks = ref(false);
const playing = ref(false);
const appInfo = ref<AppInfo>({ version: "", engineRunning: false });
const dialog = ref<{ request: DialogRequest; resolve: (value: unknown) => void } | null>(null);
const unlisteners: (() => void)[] = [];
let audio: HTMLAudioElement | null = null;
let audioUrl = "";
let playStart = 0;
let frame = 0;
let exiting = false;
let persistTimer = 0;

const TRACK_TYPES: TrackType[] = ["voice", "instrument", "audio", "midi"];

function trackTypeLabel(type: TrackType): string {
  return t(
    type === "voice" ? "trackVoice" : type === "audio" ? "trackAudio" : type === "midi" ? "trackMidi" : "trackInstrument",
  );
}

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function joinPath(folder: string, file: string): string {
  const separator = folder.includes("\\") ? "\\" : "/";
  return `${folder.replace(/[\\/]+$/, "")}${separator}${file}`;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function ask<T>(request: DialogRequest): Promise<T | null> {
  return new Promise((resolve) => {
    dialog.value = {
      request,
      resolve: (value) => {
        dialog.value = null;
        resolve(value as T | null);
      },
    };
  });
}

function closeTransient() {
  if (dialog.value) return dialog.value.resolve(null);
  showSettings.value = false;
  showExtensions.value = false;
  showVoiceCatalog.value = false;
  showSpeech.value = false;
  showVoicebanks.value = false;
}

/** Returns "save", "discard" or "cancel". Asks only when the project has unsaved changes. */
async function confirmDiscard(): Promise<"save" | "discard" | "cancel"> {
  if (!doc.dirty) return "discard";
  const choice = await ask<string>({
    kind: "message",
    title: t("dlgConfirmTitle"),
    message: t("unsavedMessage"),
    buttons: [
      { id: "save", label: t("save"), primary: true },
      { id: "discard", label: t("dontSave"), danger: true },
      { id: "cancel", label: t("cancel") },
    ],
  });
  return (choice as "save" | "discard" | "cancel" | null) ?? "cancel";
}

async function writeProject(path: string): Promise<boolean> {
  try {
    await invoke("save_project_file", { path, content: serializeProject() });
    markSaved(path);
    addRecent(path);
    logInfo(t("msgProjectSaved", baseName(path)));
    return true;
  } catch (error) {
    logError(t("saveFailed", errorText(error)));
    return false;
  }
}

async function saveProject(): Promise<boolean> {
  if (!doc.path) return saveAsProject();
  return writeProject(doc.path);
}

async function saveAsProject(): Promise<boolean> {
  const picked = await saveDialog({
    defaultPath: `${doc.project.name}.eveproj`,
    filters: [{ name: "Energy Voice Editor", extensions: ["eveproj"] }],
  });
  if (!picked) return false;
  doc.project.name = baseName(picked).replace(/\.eveproj$/i, "");
  return writeProject(picked);
}

async function newProject() {
  if ((await confirmDiscard()) === "cancel") return;
  const created = await ask<{ name: string; folder: string }>({ kind: "newProject", defaultFolder: "" });
  if (!created) return;
  const path = joinPath(created.folder, `${created.name}.eveproj`);
  resetProject(emptyProject(created.name), path);
  if (await writeProject(path)) logInfo(t("msgProjectCreated", baseName(path)));
}

async function loadProject(path: string) {
  try {
    const content = await invoke<string>("open_project_file", { path });
    resetProject(parseProject(content), path);
    addRecent(path);
    logInfo(t("msgProjectOpened", baseName(path)));
  } catch (error) {
    logError(t("msgProjectOpenFailed", errorText(error)));
  }
}

async function openProject() {
  if ((await confirmDiscard()) === "cancel") return;
  const picked = await openDialog({
    multiple: false,
    filters: [{ name: "Energy Voice Editor", extensions: ["eveproj"] }],
  });
  if (typeof picked === "string") await loadProject(picked);
}

async function openRecent(path: string) {
  if ((await confirmDiscard()) === "cancel") return;
  try {
    await loadProject(path);
  } catch {
    settings.recentProjects = settings.recentProjects.filter((item) => item !== path);
    logWarning(t("msgRecentMissing", path));
  }
}

function addRecent(path: string) {
  settings.recentProjects = [path, ...settings.recentProjects.filter((item) => item !== path)].slice(0, 10);
  persistSettings(cloneSettings(settings)).catch((error) => logError(t("saveFailed", errorText(error))));
}

async function closeProject() {
  if ((await confirmDiscard()) === "cancel") return;
  resetProject(emptyProject(), null);
  logInfo(t("msgClosed"));
}

/**
 * Used by Exit and the window close button. Asks before discarding unsaved changes, then quits through the Rust
 * core (`exit_app`), which also stops the Python worker and the console shell.
 */
async function requestClose() {
  if (exiting) return;
  if (dialog.value) dialog.value.resolve(null);
  const choice = await confirmDiscard();
  if (choice === "cancel") return;
  if (choice === "save" && !(await saveProject())) return;
  exiting = true;
  stopPlayback();
  try {
    await invoke("exit_app");
  } catch (error) {
    exiting = false;
    logError(t("logError", errorText(error)));
  }
}

/** Mute and Solo: soloed tracks play alone; muted tracks never play. */
function renderTracks() {
  const anySolo = doc.project.tracks.some((track) => track.solo);
  const audible = doc.project.tracks.filter((track) => !track.muted && (!anySolo || track.solo));
  return audible.map((track) => ({
    volume: track.volume * doc.masterVolume,
    pan: track.pan,
    instrument: track.instrument || (track.type === "voice" ? "vocal" : "piano"),
    clips: track.clips.filter((clip) => clip.path).map((clip) => ({ path: clip.path, start: clip.start })),
    notes: track.clips.flatMap((clip) => (clip.notes ?? []).map((note) => ({
      midi: note.midi, lyric: note.lyric,
      start: clip.start + note.start * 60 / (doc.project.tempo || 120),
      duration: note.duration * 60 / (doc.project.tempo || 120),
    }))),
  }));
}

async function importAudio() {
  const picked = await openDialog({
    multiple: false,
    filters: [{ name: "WAV", extensions: ["wav"] }],
  });
  if (typeof picked !== "string") return;
  try {
    const info = await invoke<AudioInfo>("import_audio", { path: picked });
    let target = selectedTrackObject();
    if (!target || target.type !== "audio") {
      const id = addTrack("audio", `${t("trackAudio")} ${doc.project.tracks.length + 1}`);
      target = doc.project.tracks.find((track) => track.id === id);
    }
    if (target) {
      addClip(target.id, {
        name: baseName(picked),
        path: info.path,
        start: doc.playhead,
        duration: info.durationSec,
      });
    }
    logInfo(t("msgImported", baseName(picked), info.durationSec.toFixed(2)));
  } catch (error) {
    logError(t("msgImportFailed", errorText(error)));
  }
}

let lastFrameTime = 0;
let lastBeat = -1;

const timeFormatted = computed(() => {
  const s = Math.max(0, doc.playhead);
  const mins = Math.floor(s / 60);
  const secs = Math.floor(s % 60);
  const ms = Math.floor((s % 1) * 1000);
  return `${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}.${String(ms).padStart(3, "0")}`;
});

const barBeatFormatted = computed(() => {
  const bpmVal = doc.project.tempo || 120;
  const beatsPerSec = bpmVal / 60;
  const totalBeats = Math.max(0, Math.floor(doc.playhead * beatsPerSec));
  const bar = Math.floor(totalBeats / 4) + 1;
  const beat = (totalBeats % 4) + 1;
  return `BAR ${bar} · BEAT ${beat}`;
});

function stopPlayback() {
  cancelAnimationFrame(frame);
  audio?.pause();
  audio = null;
  if (audioUrl) URL.revokeObjectURL(audioUrl);
  audioUrl = "";
  playing.value = false;
  lastBeat = -1;
}

function stopAndRewind() {
  stopPlayback();
  doc.playhead = 0;
}

function playbackLoop(timestamp: number) {
  if (!playing.value) return;
  if (!lastFrameTime) lastFrameTime = timestamp;
  const dt = Math.min(0.1, (timestamp - lastFrameTime) / 1000);
  lastFrameTime = timestamp;

  doc.playhead += dt * (doc.speed || 1);

  const bpmVal = doc.project.tempo || 120;
  const beatsPerSec = bpmVal / 60;

  // Check metronome click
  const currentBeat = Math.floor(doc.playhead * beatsPerSec);
  if (currentBeat !== lastBeat) {
    if (doc.metronome) {
      playMetronomeClick(currentBeat % 4 === 0);
    }
    lastBeat = currentBeat;
  }

  if (audio) doc.playhead = playStart + audio.currentTime;

  frame = requestAnimationFrame(playbackLoop);
}

async function togglePlayback() {
  if (playing.value) {
    stopPlayback();
    return;
  }
  const all = allClips();
  const hasNotes = all.some((c) => c.notes && c.notes.length > 0);
  const hasWav = all.some((c) => Boolean(c.path));
  if (!hasNotes && !hasWav) {
    return logWarning(t("msgNothingToPlay"));
  }

  if (doc.countIn) {
    playMetronomeClick(true);
  }

  {
    playStart = doc.playhead;
    try {
      const result = await invoke<RenderResult>("render_mix", {
        request: {
          sampleRate: doc.project.sampleRate,
          channels: 2,
          bitDepth: 16,
          fromSec: playStart,
          tracks: renderTracks(),
          output: null,
        },
      });
      const data = await invoke<ArrayBuffer>("read_audio", { path: result.path });
      stopPlayback();
      audioUrl = URL.createObjectURL(new Blob([data], { type: "audio/wav" }));
      audio = new Audio(audioUrl);
      audio.playbackRate = doc.speed || 1;
      audio.loop = doc.loop;
      audio.onended = () => {
        stopPlayback();
        doc.playhead = playStart;
      };
      await audio.play();
    } catch (error) {
      logError(t("msgRenderFailed", errorText(error)));
      stopPlayback();
      return;
    }
  }

  playing.value = true;
  lastFrameTime = performance.now();
  lastBeat = Math.floor(doc.playhead * ((doc.project.tempo || 120) / 60)) - 1;
  frame = requestAnimationFrame(playbackLoop);
}

function onMasterVolumeChange(event: Event) {
  const val = Number((event.target as HTMLInputElement).value);
  doc.masterVolume = val;
  setMasterVolume(val);
}

async function openExport() {
  const all = allClips();
  const hasNotes = all.some((c) => c.notes && c.notes.length > 0);
  const hasWav = all.some((c) => Boolean(c.path));
  if (!hasNotes && !hasWav) return logWarning(t("msgNothingToPlay"));

  logInfo(t("msgRendering"));
  try {
    const picked = await saveDialog({
      defaultPath: `${doc.project.name}.wav`,
      filters: [{ name: "Audio WAV", extensions: ["wav"] }],
    });
    if (!picked) return;

    const result = await invoke<RenderResult>("render_mix", {
      request: {
        sampleRate: doc.project.sampleRate,
        channels: 2,
        bitDepth: 16,
        fromSec: 0,
        tracks: renderTracks(),
        output: picked,
      },
    });
    logInfo(t("msgRenderDone", baseName(result.path), result.durationSec.toFixed(2)));
  } catch (error) {
    logError(t("msgRenderFailed", errorText(error)));
  }
}

function addTrackOf(type: TrackType) {
  const count = doc.project.tracks.filter((track) => track.type === type).length + 1;
  const name = `${trackTypeLabel(type)} ${count}`;
  addTrack(type, name, type === "voice" ? selectedVoice.value || voices.value[0]?.id : undefined);
}

function requireTrack(): Track | null {
  const track = selectedTrackObject();
  if (!track) logWarning(t("msgSelectTrack"));
  return track ?? null;
}

async function renameSelectedTrack() {
  const track = requireTrack();
  if (!track) return;
  const edited = await ask<{ name: string; volume: number; pan: number }>({
    kind: "track",
    name: track.name,
    volume: track.volume,
    pan: track.pan,
  });
  if (!edited) return;
  mutate(() => {
    track.name = edited.name;
    track.volume = edited.volume;
    track.pan = edited.pan;
  });
}

async function deleteSelectedTrack() {
  const track = requireTrack();
  if (!track) return;
  const choice = await ask<string>({
    kind: "message",
    title: t("deleteTrack"),
    message: t("deleteTrackConfirm", track.name),
    buttons: [
      { id: "cancel", label: t("cancel") },
      { id: "delete", label: t("delete"), danger: true },
    ],
  });
  if (choice === "delete") deleteTrack(track.id);
}

function moveSelectedTrack(direction: -1 | 1) {
  const track = requireTrack();
  if (track) moveTrack(track.id, direction);
}

function editTrackFromHeader(id: string) {
  doc.selectedTrack = id;
  void renameSelectedTrack();
}

function duplicateSelected() {
  if (doc.selectedClips.length > 0) return duplicateSelectedClips();
  const track = requireTrack();
  if (track) duplicateTrack(track.id);
}

function deleteSelected() {
  if (doc.selectedClips.length > 0) {
    const count = doc.selectedClips.length;
    deleteSelectedClips();
    return logInfo(t("msgDeleted", count));
  }
  if (selectedTrackObject()) return void deleteSelectedTrack();
  logWarning(t("msgSelectClip"));
}

function paste(options: { time?: number; trackId?: string } = {}) {
  const count = pasteClips(options);
  if (count > 0) logInfo(t("msgPasted", count));
}

function cut() {
  if (doc.selectedClips.length === 0) return logWarning(t("msgSelectClip"));
  cutSelectedClips();
}

function copy() {
  if (doc.selectedClips.length === 0) return logWarning(t("msgSelectClip"));
  copySelectedClips();
}

function clampSize(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function onHeadResize(delta: number) {
  ws.trackPanelWidth = clampSize(ws.trackPanelWidth + delta, 160, 600);
}

function onBottomResize(delta: number) {
  ws.bottomHeight = clampSize(ws.bottomHeight - delta, 160, window.innerHeight * 0.8);
}

function onTermResize(delta: number) {
  ws.terminalWidth = clampSize(ws.terminalWidth - delta, 200, window.innerWidth * 0.6);
}

function persistWorkspace() {
  persistSettings(cloneSettings(settings)).catch((error) => logError(t("saveFailed", errorText(error))));
}

const BASE_WIDTH = 1920;
const BASE_HEIGHT = 1080;

/** Scales the whole UI to the window: 1920x1080 logical pixels is 100%, clamped to 80%..150%. */
async function applyScale() {
  const win = getCurrentWindow();
  const physical = await win.innerSize();
  const dpi = await win.scaleFactor();
  const width = physical.width / dpi;
  const height = physical.height / dpi;
  const factor = clampSize(Math.min(width / BASE_WIDTH, height / BASE_HEIGHT), 0.8, 1.5);
  await getCurrentWebview().setZoom(factor);
}

function setWorkspace(key: "trackPanel" | "pianoRoll" | "terminal") {
  ws[key] = !ws[key];
  persistSettings(cloneSettings(settings)).catch((error) => logError(t("saveFailed", errorText(error))));
}

/** Zoom changes often (mouse wheel), so the settings file is written once the zooming has stopped. */
function setZoom(px: number) {
  ws.pxPerSec = Math.min(400, Math.max(10, px));
  window.clearTimeout(persistTimer);
  persistTimer = window.setTimeout(persistWorkspace, 400);
}

function zoomFit() {
  const available = window.innerWidth - (ws.trackPanel ? 300 : 0) - 40;
  setZoom(available / Math.max(10, timelineEnd()));
}

async function toggleFullscreen() {
  const win = getCurrentWindow();
  await win.setFullscreen(!(await win.isFullscreen()));
}

function resetWorkspace() {
  Object.assign(ws, { ...DEFAULT_WORKSPACE });
  persistSettings(cloneSettings(settings)).catch((error) => logError(t("saveFailed", errorText(error))));
  logInfo(t("msgWorkspaceReset"));
}

async function showAbout() {
  await ask({ kind: "about", version: appInfo.value.version, engineRunning: appInfo.value.engineRunning });
}

async function showShortcuts() {
  const rows = (Object.keys(SHORTCUTS) as ActionId[]).map((id) => ({ label: shortcutLabel(id), keys: SHORTCUTS[id] }));
  await ask({ kind: "shortcuts", rows });
}

function shortcutLabel(id: ActionId): string {
  const labels: Record<ActionId, string> = {
    newProject: t("newProject"),
    openProject: t("openProject"),
    save: t("save"),
    saveAs: t("saveAs"),
    undo: t("undo"),
    redo: t("redo"),
    cut: t("cut"),
    copy: t("copy"),
    paste: t("paste"),
    duplicate: t("duplicate"),
    delete: t("delete"),
    selectAll: t("selectAll"),
    playPause: t("playClipTitle"),
    escape: t("close"),
  };
  return labels[id];
}

function runAction(id: ActionId) {
  if (dialog.value || showSettings.value || showVoicebanks.value || showExtensions.value || showVoiceCatalog.value || showSpeech.value) {
    if (id === "escape") closeTransient();
    return;
  }
  switch (id) {
    case "newProject":
      return void newProject();
    case "openProject":
      return void openProject();
    case "save":
      return void saveProject();
    case "saveAs":
      return void saveAsProject();
    case "undo":
      undo();
      return;
    case "redo":
      redo();
      return;
    case "cut":
      return cut();
    case "copy":
      return copy();
    case "paste":
      return paste();
    case "duplicate":
      return duplicateSelected();
    case "delete":
      return deleteSelected();
    case "selectAll":
      selectAllClips();
      return;
    case "playPause":
      return void togglePlayback();
    case "escape":
      closeTransient();
      return;
  }
}

function onKeydown(event: KeyboardEvent) {
  dispatchShortcut(event, runAction);
}

/* ---------- voices ---------- */

async function loadVoices(refresh: boolean) {
  voices.value = await invoke<Voice[]>("list_voices", { refresh });
  if (!voices.value.some((item) => item.id === selectedVoice.value)) selectedVoice.value = voices.value[0]?.id ?? "";
}

async function refreshVoices() {
  try {
    await loadVoices(true);
    logInfo(t("msgVoicesRefreshed", voices.value.length));
  } catch (error) {
    logError(t("logError", errorText(error)));
  }
}

async function openVoicesFolder() {
  try {
    const folder = await invoke<string>("get_voices_dir");
    logInfo(t("msgVoicesFolder", folder));
    await revealItemInDir(folder);
  } catch (error) {
    logError(t("logError", errorText(error)));
  }
}

watch(
  () => doc.selectedTrack,
  () => {
    const track = selectedTrackObject();
    if (track?.type === "voice" && track.voiceId && voices.value.some((item) => item.id === track.voiceId)) {
      selectedVoice.value = track.voiceId;
    }
  },
);

/* ---------- context menus ---------- */

const SEP: MenuItem = { separator: true };

function addTrackItems(): MenuItem[] {
  return TRACK_TYPES.map((type) => ({ label: trackTypeLabel(type), action: () => addTrackOf(type) }));
}

function openAddTrackMenu(x: number, y: number) {
  showContextMenu(x, y, addTrackItems());
}

function playFrom(time: number) {
  stopPlayback();
  doc.playhead = time;
  void togglePlayback();
}

function zoomItems(): MenuItem[] {
  return [
    { label: t("zoomIn"), action: () => setZoom(ws.pxPerSec * 1.25) },
    { label: t("zoomOut"), action: () => setZoom(ws.pxPerSec / 1.25) },
    { label: t("zoomFit"), action: zoomFit },
  ];
}

function clipEditItems(): MenuItem[] {
  const hasClips = doc.selectedClips.length > 0;
  return [
    { label: t("cut"), shortcut: SHORTCUTS.cut, disabled: !hasClips, action: cut },
    { label: t("copy"), shortcut: SHORTCUTS.copy, disabled: !hasClips, action: copy },
    { label: t("duplicate"), shortcut: SHORTCUTS.duplicate, disabled: !hasClips, action: duplicateSelected },
    { label: t("delete"), shortcut: SHORTCUTS.delete, disabled: !hasClips, danger: true, action: deleteSelected },
  ];
}

function trackMenu(track: Track): MenuItem[] {
  const list = doc.project.tracks;
  const index = list.indexOf(track);
  const heights: [TrackHeight, string][] = [
    ["compact", t("heightCompact")],
    ["normal", t("heightNormal")],
    ["large", t("heightLarge")],
  ];
  const items: MenuItem[] = [
    { label: t("trackProperties"), action: renameSelectedTrack },
    {
      label: t("trackColor"),
      submenu: [
        { palette: { colors: TRACK_PALETTE, current: track.color, pick: (color) => setTrackColor(track.id, color) } },
      ],
    },
    {
      label: t("trackHeight"),
      submenu: heights.map(([height, label]) => ({
        label,
        checked: track.height === height,
        action: () => setTrackHeight(track.id, height),
      })),
    },
  ];
  if (track.type === "voice" && voices.value.length > 0) {
    items.push({
      label: t("trackVoiceMenu"),
      submenu: voices.value.map((voice) => ({
        label: voice.name,
        checked: track.voiceId === voice.id,
        action: () => {
          setTrackVoice(track.id, voice.id);
          selectedVoice.value = voice.id;
        },
      })),
    });
  }
  items.push(
    SEP,
    { label: t("mute"), checked: track.muted, action: () => toggleTrackFlag(track.id, "muted") },
    { label: t("solo"), checked: track.solo, action: () => toggleTrackFlag(track.id, "solo") },
  );
  if (list.some((item) => item.solo)) items.push({ label: t("clearSolo"), action: clearSolo });
  items.push(
    SEP,
    {
      label: t("selectTrackClips"),
      disabled: track.clips.length === 0,
      action: () => (doc.selectedClips = track.clips.map((clip) => clip.id)),
    },
    { label: t("clearTrack"), disabled: track.clips.length === 0, danger: true, action: () => clearTrack(track.id) },
    SEP,
    { label: t("addTrack"), submenu: addTrackItems() },
    { label: t("duplicateTrack"), action: () => duplicateTrack(track.id) },
    { label: t("moveUp"), disabled: index <= 0, action: () => moveTrack(track.id, -1) },
    { label: t("moveDown"), disabled: index < 0 || index >= list.length - 1, action: () => moveTrack(track.id, 1) },
    SEP,
    { label: t("deleteTrack"), danger: true, action: deleteSelectedTrack },
  );
  return items;
}

async function revealClip(path: string) {
  try {
    await revealItemInDir(path);
  } catch (error) {
    logError(t("logError", errorText(error)));
  }
}

function clipMenu(trackId: string, clipId: string): MenuItem[] {
  const clip = allClips().find((item) => item.id === clipId);
  if (!clip) return [];
  const canPaste = doc.clipboard.length > 0 && acceptsClips(doc.project.tracks.find((item) => item.id === trackId)?.type ?? "midi");
  const text = clip.text;
  return [
    { label: t("playFromHere"), action: () => playFrom(clip.start) },
    SEP,
    ...clipEditItems().slice(0, 2),
    { label: t("paste"), shortcut: SHORTCUTS.paste, disabled: !canPaste, action: () => paste({ trackId }) },
    ...clipEditItems().slice(2),
    SEP,
    {
      label: t("copyClipText"),
      disabled: !text,
      action: () => void navigator.clipboard.writeText(text ?? "").catch((error) => logError(t("logError", errorText(error)))),
    },
    { label: t("revealInFolder"), action: () => void revealClip(clip.path) },
  ];
}

function laneMenu(trackId: string, time: number): MenuItem[] {
  const track = doc.project.tracks.find((item) => item.id === trackId);
  if (!track) return [];
  return [
    {
      label: t("paste"),
      shortcut: SHORTCUTS.paste,
      disabled: doc.clipboard.length === 0 || !acceptsClips(track.type),
      action: () => paste({ trackId, time }),
    },
    SEP,
    { label: t("setPlayheadHere"), action: () => (doc.playhead = time) },
    { label: t("playFromHere"), action: () => playFrom(time) },
    SEP,
    {
      label: t("selectTrackClips"),
      disabled: track.clips.length === 0,
      action: () => (doc.selectedClips = track.clips.map((clip) => clip.id)),
    },
    { label: t("addTrack"), submenu: addTrackItems() },
  ];
}

function rulerMenu(time: number): MenuItem[] {
  return [
    { label: t("setPlayheadHere"), action: () => (doc.playhead = time) },
    { label: t("playFromHere"), action: () => playFrom(time) },
    SEP,
    ...zoomItems(),
  ];
}

function emptyMenu(): MenuItem[] {
  return [
    { label: t("addTrack"), submenu: addTrackItems() },
    { label: t("paste"), shortcut: SHORTCUTS.paste, disabled: doc.clipboard.length === 0, action: () => paste() },
    { label: t("selectAll"), shortcut: SHORTCUTS.selectAll, disabled: allClips().length === 0, action: selectAllClips },
    SEP,
    ...zoomItems(),
  ];
}

function onArrangementContext(request: ArrangementContext & { x: number; y: number }) {
  let items: MenuItem[] = [];
  if (request.kind === "track") {
    const track = doc.project.tracks.find((item) => item.id === request.trackId);
    items = track ? trackMenu(track) : [];
  } else if (request.kind === "clip") items = clipMenu(request.trackId, request.clipId);
  else if (request.kind === "lane") items = laneMenu(request.trackId, request.time);
  else if (request.kind === "ruler") items = rulerMenu(request.time);
  else items = emptyMenu();
  showContextMenu(request.x, request.y, items);
}

/** Replaces the browser's own right-click menu (Reload, Inspect...). Text fields keep the native Cut/Copy/Paste menu. */
function onNativeContextMenu(event: MouseEvent) {
  if (isTextInput(event.target)) {
    hideContextMenu();
    return;
  }
  event.preventDefault();
}

const menus = computed<MenuDef[]>(() => [
  {
    label: t("file"),
    items: [
      { label: t("newProject"), shortcut: SHORTCUTS.newProject, action: newProject },
      { label: t("openProject"), shortcut: SHORTCUTS.openProject, action: openProject },
      {
        label: t("openRecent"),
        submenu: settings.recentProjects.length
          ? settings.recentProjects.map((path) => ({ label: baseName(path), action: () => openRecent(path) }))
          : [{ label: t("msgRecentEmpty"), disabled: true }],
      },
      { separator: true },
      { label: t("save"), shortcut: SHORTCUTS.save, action: saveProject },
      { label: t("saveAs"), shortcut: SHORTCUTS.saveAs, action: saveAsProject },
      { separator: true },
      { label: t("importAudio"), action: importAudio },
      { label: t("importMidi"), disabled: true },
      { label: t("exportAudio"), action: openExport },
      { label: t("exportMidi"), disabled: true },
      { separator: true },
      { label: t("projectSettings"), disabled: true },
      { label: t("closeProject"), action: closeProject },
      { label: t("exit"), action: requestClose },
    ],
  },
  {
    label: t("edit"),
    items: [
      { label: t("undo"), shortcut: SHORTCUTS.undo, disabled: !canUndo(), action: () => undo() },
      { label: t("redo"), shortcut: SHORTCUTS.redo, disabled: !canRedo(), action: () => redo() },
      { separator: true },
      { label: t("cut"), shortcut: SHORTCUTS.cut, action: cut },
      { label: t("copy"), shortcut: SHORTCUTS.copy, action: copy },
      { label: t("paste"), shortcut: SHORTCUTS.paste, action: paste },
      { label: t("duplicate"), shortcut: SHORTCUTS.duplicate, action: duplicateSelected },
      { label: t("delete"), shortcut: SHORTCUTS.delete, action: deleteSelected },
      { separator: true },
      { label: t("selectAll"), shortcut: SHORTCUTS.selectAll, action: selectAllClips },
      { separator: true },
      { label: t("preferences"), action: () => (showSettings.value = true) },
      { label: t("keyboardShortcuts"), action: showShortcuts },
    ],
  },
  {
    label: t("object"),
    items: [
      {
        label: t("addTrack"),
        submenu: TRACK_TYPES.map((type) => ({ label: trackTypeLabel(type), action: () => addTrackOf(type) })),
      },
      { label: t("duplicateTrack"), action: duplicateSelected },
      { label: t("renameTrack"), action: renameSelectedTrack },
      { label: t("deleteTrack"), action: deleteSelectedTrack },
      { separator: true },
      { label: t("moveUp"), action: () => moveSelectedTrack(-1) },
      { label: t("moveDown"), action: () => moveSelectedTrack(1) },
      { label: t("trackProperties"), action: renameSelectedTrack },
      { separator: true },
      { label: t("addMarker"), disabled: true },
      { label: t("deleteMarker"), disabled: true },
    ],
  },
  {
    label: t("view"),
    items: [
      { label: t("showTrackPanel"), checked: ws.trackPanel, action: () => setWorkspace("trackPanel") },
      { label: t("showPianoRoll"), checked: ws.pianoRoll, action: () => setWorkspace("pianoRoll") },
      { label: t("showTerminal"), checked: ws.terminal, action: () => setWorkspace("terminal") },
      { label: t("showInspector"), disabled: true },
      { label: t("showBrowser"), disabled: true },
      { separator: true },
      { label: t("zoomIn"), action: () => setZoom(ws.pxPerSec * 1.25) },
      { label: t("zoomOut"), action: () => setZoom(ws.pxPerSec / 1.25) },
      { label: t("zoomFit"), action: zoomFit },
      { separator: true },
      { label: t("fullscreen"), action: toggleFullscreen },
      { label: t("resetWorkspace"), action: resetWorkspace },
      { label: t("workspaceLayouts"), disabled: true },
    ],
  },
  {
    label: t("settings"),
    items: [
      { label: t("vcTitle"), action: () => (showVoiceCatalog.value = true) },
      { label: t("extensions"), action: () => (showExtensions.value = true) },
      { label: t("voicebankManager") + "...", action: () => (showVoicebanks.value = true) },
      { label: t("preferences"), action: () => (showSettings.value = true) },
    ],
  },
  {
    label: t("help"),
    items: [
      { label: t("refreshVoices"), action: refreshVoices },
      { label: t("openVoicesFolder"), action: openVoicesFolder },
      { separator: true },
      { label: t("documentation"), disabled: true },
      { label: t("gettingStarted"), disabled: true },
      { label: t("keyboardShortcuts"), action: showShortcuts },
      { label: t("diagnostics"), action: showAbout },
      { label: t("checkUpdates"), disabled: true },
      { label: t("reportBug"), disabled: true },
      { separator: true },
      { label: t("about"), action: showAbout },
    ],
  },
]);

watchEffect(() => applySettings(settings));

onMounted(async () => {
  refreshInstruments().catch(error => logError(String(error)));
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("contextmenu", onNativeContextMenu, true);
  unlisteners.push(
    await listen<number>("synthesis-progress", (event) => (progress.value = event.payload)),
    await listen<string>("worker-log", (event) => logInfo(event.payload, "worker")),
  );
  await applyScale();
  unlisteners.push(await getCurrentWindow().onResized(() => void applyScale()));
  // Always take over closing: the window is closed by the Rust core (exit_app) after the unsaved-changes check.
  const closeUnlisten = await getCurrentWindow().onCloseRequested(async (event) => {
    event.preventDefault();
    await requestClose();
  });
  unlisteners.push(closeUnlisten);
  const settingsError = await loadSettings();
  if (settingsError) logError(t("logSettingsLoadFailed", settingsError));
  try {
    appInfo.value = await invoke<AppInfo>("get_app_info");
    await loadVoices(false);
    logInfo(t("logLoadedVoices", voices.value.length));
  } catch (error) {
    logError(t("logError", errorText(error)));
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("contextmenu", onNativeContextMenu, true);
  window.clearTimeout(persistTimer);
  unlisteners.forEach((unlisten) => unlisten());
  stopPlayback();
});
</script>

<template>
  <div class="app" :style="{ '--term-w': `${ws.terminalWidth}px`, '--bottom-h': `${ws.bottomHeight}px` }">
    <!-- Top Menu Bar with Logo & Menus -->
    <MenuBar :menus="menus" :status-text="playing ? 'Playing' : 'Ready'" />

    <!-- Transport Bar -->
    <header class="transport-panel">
      <!-- Stop, Play, Loop -->
      <div class="transport-group">
        <button class="transport-btn stop" title="Stop (Enter)" @click="stopAndRewind">■</button>
        <button
          class="transport-btn play"
          :class="{ playing }"
          title="Play / Pause (Space)"
          @click="togglePlayback"
        >
          {{ playing ? "❚❚" : "▶" }}
        </button>
        <button
          class="transport-btn loop"
          :class="{ active: doc.loop }"
          title="Toggle Loop"
          @click="doc.loop = !doc.loop"
        >⟳</button>
      </div>

      <div class="transport-separator"></div>

      <!-- Time display -->
      <div class="time-display-box">
        <div class="time-val">{{ timeFormatted }}</div>
        <div class="pos-val">{{ barBeatFormatted }}</div>
      </div>

      <!-- Tempo, Sig, Speed -->
      <div class="transport-params">
        <div class="param-item">
          <span class="param-label">TEMPO</span>
          <input
            v-model.number="doc.project.tempo"
            type="number"
            min="30"
            max="300"
            class="param-input"
          />
        </div>
        <div class="param-item">
          <span class="param-label">SIG</span>
          <span class="param-val">4/4</span>
        </div>
        <div class="param-item">
          <span class="param-label">SPEED</span>
          <span
            class="param-val clickable"
            title="Click to toggle playback speed"
            @click="doc.speed = doc.speed === 1 ? 1.5 : doc.speed === 1.5 ? 0.5 : 1"
          >
            {{ doc.speed || 1 }}×
          </span>
        </div>
      </div>

      <div class="transport-separator"></div>

      <!-- Metronome & Count-in -->
      <div class="transport-group">
        <button
          class="transport-btn toggle"
          :class="{ active: doc.metronome }"
          title="Metronome clicks on beats"
          @click="doc.metronome = !doc.metronome"
        >
          Metronome
        </button>
        <button
          class="transport-btn toggle"
          :class="{ active: doc.countIn }"
          title="Count-in 1 bar before play"
          @click="doc.countIn = !doc.countIn"
        >
          Count-in
        </button>
      </div>

      <div class="transport-separator"></div>

      <!-- Undo / Redo -->
      <div class="transport-group">
        <button class="transport-btn icon" :disabled="!canUndo()" title="Undo (Ctrl+Z)" @click="undo">↶</button>
        <button class="transport-btn icon" :disabled="!canRedo()" title="Redo (Ctrl+Y)" @click="redo">↷</button>
      </div>

      <div class="transport-separator"></div>

      <!-- Master volume -->
      <div class="master-volume-box">
        <span class="master-label">MASTER</span>
        <div class="slider-wrapper">
          <input
            type="range"
            min="0"
            max="1"
            step="0.01"
            :value="doc.masterVolume ?? 0.8"
            class="master-slider"
            @input="onMasterVolumeChange"
          />
        </div>
      </div>

      <div class="spacer"></div>

      <button class="btn-export" @click="showSpeech = true">{{ t("speechTitle") }}</button>
      <!-- Export button -->
      <button class="btn-export" title="Export song to WAV audio" @click="openExport">Export</button>
    </header>

    <!-- Main Arrangement View (Tracks + Timeline) -->
    <main class="arrange">
      <Arrangement
        :px-per-sec="ws.pxPerSec"
        :track-panel="ws.trackPanel"
        :track-panel-width="ws.trackPanelWidth"
        :tool="tool"
        :snap="snap"
        :grid-sec="gridSec"
        @track-dblclick="editTrackFromHeader"
        @head-resize="onHeadResize"
        @head-resize-end="persistWorkspace"
        @context="onArrangementContext"
        @add-track-menu="openAddTrackMenu"
        @zoom="setZoom"
      />
    </main>

    <Splitter direction="y" @move="onBottomResize" @end="persistWorkspace" />

    <!-- Bottom Section: Piano Roll & Console -->
    <section class="bottom">
      <PianoRoll
        v-if="ws.pianoRoll"
        class="piano-pane"
        :tempo="doc.project.tempo || 120"
      />
      <Splitter v-if="ws.pianoRoll && ws.terminal" direction="x" @move="onTermResize" @end="persistWorkspace" />
      <Terminal
        v-show="ws.terminal"
        class="term-pane"
        @hide="ws.terminal = false"
      />
    </section>

    <VoicebankManager v-if="showVoicebanks" @close="showVoicebanks = false" />
    <SpeechStudio v-if="showSpeech" @close="showSpeech = false" />
    <VoiceCatalog v-if="showVoiceCatalog" @close="showVoiceCatalog = false" @installed="refreshVoices" />
    <ExtensionsDialog v-if="showExtensions" @close="showExtensions = false" />
    <SettingsDialog v-if="showSettings" @close="showSettings = false" @saved="showSettings = false" />
    <AppDialog v-if="dialog" :request="dialog.request" @done="dialog.resolve($event)" />
    <ContextMenu />
  </div>
</template>

<style>
:root {
  font-family: Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  color: var(--text);
  background-color: var(--background);
}

html,
body,
#app {
  height: 100%;
  margin: 0;
  padding: 0;
  background: var(--background);
}

body {
  overflow: hidden;
  -webkit-font-smoothing: antialiased;
}

:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track,
::-webkit-scrollbar-corner {
  background: transparent;
}

::-webkit-scrollbar-thumb {
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.15);
}

::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.3);
}

/* Base app layout matching .figcraft: 8px outer margins and 8px gaps */
.app {
  height: 100vh;
  box-sizing: border-box;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: var(--background);
  color: var(--text);
  overflow: hidden;
}

/* Transport panel */
.transport-panel {
  height: 64px;
  flex: none;
  background: var(--panel);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  padding: 0 16px;
  display: flex;
  align-items: center;
  gap: 12px;
  box-sizing: border-box;
  user-select: none;
}

.transport-group {
  display: flex;
  align-items: center;
  gap: 6px;
}

.transport-separator {
  width: 1px;
  height: 28px;
  background: rgba(255, 255, 255, 0.08);
}

.transport-btn {
  height: 36px;
  padding: 0 14px;
  border: none;
  border-radius: 8px;
  background: var(--button);
  color: var(--text);
  font-family: Inter, sans-serif;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s ease;
}

.transport-btn:hover:not(:disabled) {
  background: var(--bar);
  color: #ffffff;
}

.transport-btn.stop {
  width: 36px;
  padding: 0;
  font-size: 14px;
}

.transport-btn.play {
  width: 44px;
  padding: 0;
  background: var(--accent);
  color: #1a1405;
  font-size: 16px;
  font-weight: 800;
  box-shadow: 0 2px 8px rgba(255, 209, 102, 0.35);
}

.transport-btn.play:hover {
  background: #ffe082;
}

.transport-btn.play.playing {
  background: #ffb74d;
}

.transport-btn.loop.active,
.transport-btn.toggle.active {
  background: rgba(255, 209, 102, 0.18);
  color: var(--accent);
  border: 1px solid rgba(255, 209, 102, 0.35);
}

.transport-btn.icon {
  width: 36px;
  padding: 0;
  font-size: 15px;
}

.transport-btn:disabled {
  opacity: 0.35;
  cursor: default;
}

/* Time display box */
.time-display-box {
  background: #0b0d12;
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
  padding: 4px 14px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  font-family: "JetBrains Mono", monospace;
}

.time-val {
  font-size: 16px;
  font-weight: 700;
  color: var(--text);
  letter-spacing: -0.3px;
}

.pos-val {
  font-size: 9px;
  font-weight: 600;
  color: color-mix(in srgb, var(--text) 60%, transparent);
  letter-spacing: 0.5px;
}

/* Parameters */
.transport-params {
  display: flex;
  align-items: center;
  gap: 16px;
}

.param-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.param-label {
  font-size: 9px;
  font-weight: 700;
  color: color-mix(in srgb, var(--text) 60%, transparent);
  letter-spacing: 0.5px;
}

.param-val {
  font-family: "JetBrains Mono", monospace;
  font-size: 13px;
  font-weight: 700;
  color: var(--text);
}

.param-val.clickable {
  cursor: pointer;
  color: var(--accent);
}

.param-input {
  width: 52px;
  height: 20px;
  background: var(--button);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  color: var(--text);
  font-family: "JetBrains Mono", monospace;
  font-size: 13px;
  font-weight: 700;
  padding: 0 4px;
  outline: none;
}

/* Master volume */
.master-volume-box {
  display: flex;
  flex-direction: column;
  gap: 4px;
  width: 110px;
}

.master-label {
  font-size: 9px;
  font-weight: 700;
  color: color-mix(in srgb, var(--text) 60%, transparent);
  letter-spacing: 0.5px;
}

.slider-wrapper {
  display: flex;
  align-items: center;
}

.master-slider {
  width: 100%;
  height: 4px;
  -webkit-appearance: none;
  background: #2b3145;
  border-radius: 2px;
  outline: none;
  cursor: pointer;
}

.master-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #ffffff;
  box-shadow: 0 0 4px rgba(0, 0, 0, 0.5);
  cursor: pointer;
  transition: transform 0.1s ease;
}

.master-slider::-webkit-slider-thumb:hover {
  transform: scale(1.2);
}

.spacer {
  flex: 1;
}

/* Export button */
.btn-export {
  height: 38px;
  padding: 0 20px;
  border: none;
  border-radius: 8px;
  background: var(--accent);
  color: #1a1405;
  font-family: Inter, sans-serif;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
  box-shadow: 0 2px 10px rgba(255, 209, 102, 0.35);
  transition: all 0.15s ease;
}

.btn-export:hover {
  background: #ffe082;
  box-shadow: 0 4px 14px rgba(255, 209, 102, 0.5);
}

/* Arrangement Middle Section */
.arrange {
  flex: 1;
  min-height: 0;
  background: var(--panel);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  overflow: hidden;
}

/* Bottom Section */
.bottom {
  display: flex;
  height: var(--bottom-h);
  min-height: 160px;
  gap: 8px;
}

.piano-pane {
  flex: 1;
  min-width: 0;
  height: 100%;
}

.term-pane {
  width: var(--term-w);
  flex: none;
  height: 100%;
}
</style>
