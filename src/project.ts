import { reactive } from "vue";
import { isHexColor, pickDefaultColor, TRACK_PALETTE } from "./trackColors";

export type TrackType = "voice" | "audio" | "instrument" | "midi";

export interface Note {
  id: string;
  midi: number;
  start: number;
  duration: number;
  lyric: string;
}

export interface ExpressionPoint {
  time: number;
  value: number;
}

export interface Clip {
  id: string;
  name: string;
  /** Absolute path to a WAV file on disk (optional for synthesized vocal/note clips). */
  path: string;
  start: number;
  duration: number;
  text?: string;
  notes?: Note[];
  curves?: Record<string, ExpressionPoint[]>;
}

export type TrackHeight = "compact" | "normal" | "large";

export interface Track {
  id: string;
  type: TrackType;
  name: string;
  voiceId?: string;
  instrument?: string;
  /** Track colour as #RRGGBB. Older project files without a colour get one when they are opened. */
  color: string;
  height: TrackHeight;
  volume: number;
  pan: number;
  muted: boolean;
  solo: boolean;
  clips: Clip[];
}

export interface Project {
  version: 1;
  name: string;
  sampleRate: number;
  tempo?: number;
  timeSignature?: [number, number];
  tracks: Track[];
}

const MAX_HISTORY = 100;

/** Row height in pixels for each track height setting. Headers and lanes both read this, so they stay aligned. */
export const ROW_HEIGHTS: Record<TrackHeight, number> = { compact: 40, normal: 60, large: 96 };

export function trackRowHeight(track: Track): number {
  return ROW_HEIGHTS[track.height] ?? ROW_HEIGHTS.normal;
}

/** Only voice, audio and instrument tracks hold clips. */
export function acceptsClips(type: TrackType): boolean {
  return type === "voice" || type === "audio" || type === "instrument" || type === "midi";
}

export function emptyProject(name = "Untitled"): Project {
  return {
    version: 1,
    name,
    sampleRate: 22050,
    tempo: 120,
    timeSignature: [4, 4],
    tracks: [
      {
        id: "track-1",
        type: "voice",
        name: "Voice 1",
        color: TRACK_PALETTE[0],
        height: "normal",
        volume: 0.8,
        pan: 0,
        muted: false,
        solo: false,
        clips: [],
      },
    ],
  };
}

export const doc = reactive({
  project: emptyProject(),
  path: null as string | null,
  dirty: false,
  selectedTrack: null as string | null,
  selectedClips: [] as string[],
  selectedNote: null as string | null,
  activeClipId: null as string | null,
  clipboard: [] as Clip[],
  playhead: 0,
  loop: false,
  loopStart: 0,
  loopEnd: 16,
  metronome: false,
  countIn: false,
  speed: 1,
  masterVolume: 0.8,
});

const history = reactive({ past: [] as string[], future: [] as string[] });

export const canUndo = () => history.past.length > 0;
export const canRedo = () => history.future.length > 0;

function snapshot(): string {
  return JSON.stringify(doc.project);
}

/** Call before a change that should be undoable. Continuous edits (sliders, drags) call it once at the start. */
export function beginChange() {
  history.past.push(snapshot());
  if (history.past.length > MAX_HISTORY) history.past.shift();
  history.future = [];
  doc.dirty = true;
}

/** Runs an undoable change. */
export function mutate(change: () => void) {
  beginChange();
  change();
  pruneSelection();
}

export function undo(): boolean {
  const previous = history.past.pop();
  if (previous === undefined) return false;
  history.future.push(snapshot());
  doc.project = JSON.parse(previous);
  doc.dirty = true;
  pruneSelection();
  return true;
}

export function redo(): boolean {
  const next = history.future.pop();
  if (next === undefined) return false;
  history.past.push(snapshot());
  doc.project = JSON.parse(next);
  doc.dirty = true;
  pruneSelection();
  return true;
}

/** Replaces the whole project (open, new, close). Clears history and the dirty flag. */
export function resetProject(project: Project, path: string | null) {
  doc.project = project;
  doc.path = path;
  doc.dirty = false;
  doc.selectedTrack = null;
  doc.selectedClips = [];
  doc.activeClipId = null;
  doc.selectedNote = null;
  doc.playhead = 0;
  history.past = [];
  history.future = [];
}

export function markSaved(path: string) {
  doc.path = path;
  doc.dirty = false;
}

function pruneSelection() {
  const clipIds = new Set(allClips().map((clip) => clip.id));
  doc.selectedClips = doc.selectedClips.filter((id) => clipIds.has(id));
  if (doc.selectedTrack && !doc.project.tracks.some((track) => track.id === doc.selectedTrack)) {
    doc.selectedTrack = null;
  }
}

export function allClips(): Clip[] {
  return doc.project.tracks.flatMap((track) => track.clips);
}

export function trackOf(clipId: string): Track | undefined {
  return doc.project.tracks.find((track) => track.clips.some((clip) => clip.id === clipId));
}

export function selectedTrackObject(): Track | undefined {
  return doc.project.tracks.find((track) => track.id === doc.selectedTrack);
}

export function timelineEnd(): number {
  return allClips().reduce((end, clip) => Math.max(end, clip.start + clip.duration), 0);
}

export function trackEnd(track: Track): number {
  return track.clips.reduce((end, clip) => Math.max(end, clip.start + clip.duration), 0);
}

/** Plain copy of reactive state. structuredClone cannot clone Vue proxies. */
function plain<T>(value: T): T {
  return JSON.parse(JSON.stringify(value));
}

export function newId(): string {
  return crypto.randomUUID();
}

export function addTrack(type: TrackType, name: string, voiceId?: string): string {
  const track: Track = {
    id: newId(),
    type,
    name,
    voiceId,
    color: pickDefaultColor(doc.project.tracks.map((item) => item.color)),
    height: "normal",
    volume: 1,
    pan: 0,
    muted: false,
    solo: false,
    clips: [],
  };
  mutate(() => doc.project.tracks.push(track));
  doc.selectedTrack = track.id;
  return track.id;
}

export function renameTrack(id: string, name: string) {
  mutate(() => {
    const track = doc.project.tracks.find((item) => item.id === id);
    if (track) track.name = name;
  });
}

export function setTrackColor(id: string, color: string) {
  if (!isHexColor(color)) return;
  mutate(() => {
    const track = doc.project.tracks.find((item) => item.id === id);
    if (track) track.color = color.toUpperCase();
  });
}

export function setTrackHeight(id: string, height: TrackHeight) {
  mutate(() => {
    const track = doc.project.tracks.find((item) => item.id === id);
    if (track) track.height = height;
  });
}

export function setTrackVoice(id: string, voiceId: string) {
  mutate(() => {
    const track = doc.project.tracks.find((item) => item.id === id);
    if (track) track.voiceId = voiceId;
  });
}

export function toggleTrackFlag(id: string, key: "muted" | "solo") {
  mutate(() => {
    const track = doc.project.tracks.find((item) => item.id === id);
    if (track) track[key] = !track[key];
  });
}

/** Turns off Solo on every track. */
export function clearSolo() {
  mutate(() => {
    for (const track of doc.project.tracks) track.solo = false;
  });
}

/** Removes every clip from one track but keeps the track itself. */
export function clearTrack(id: string) {
  mutate(() => {
    const track = doc.project.tracks.find((item) => item.id === id);
    if (track) track.clips = [];
  });
}

export function deleteTrack(id: string) {
  mutate(() => {
    doc.project.tracks = doc.project.tracks.filter((track) => track.id !== id);
  });
}

export function duplicateTrack(id: string) {
  const index = doc.project.tracks.findIndex((track) => track.id === id);
  if (index < 0) return;
  const source = doc.project.tracks[index];
  const copy: Track = {
    ...plain(source),
    id: newId(),
    name: `${source.name} copy`,
    clips: source.clips.map((clip) => ({ ...plain(clip), id: newId() })),
  };
  mutate(() => doc.project.tracks.splice(index + 1, 0, copy));
  doc.selectedTrack = copy.id;
}

export function moveTrack(id: string, direction: -1 | 1) {
  const index = doc.project.tracks.findIndex((track) => track.id === id);
  const target = index + direction;
  if (index < 0 || target < 0 || target >= doc.project.tracks.length) return;
  mutate(() => {
    const [track] = doc.project.tracks.splice(index, 1);
    doc.project.tracks.splice(target, 0, track);
  });
}

/** Moves a track so that it ends up at `index` in the final order (0 = first). */
export function moveTrackTo(id: string, index: number) {
  const from = doc.project.tracks.findIndex((track) => track.id === id);
  const to = Math.max(0, Math.min(doc.project.tracks.length - 1, index));
  if (from < 0 || from === to) return;
  mutate(() => {
    const [track] = doc.project.tracks.splice(from, 1);
    doc.project.tracks.splice(to, 0, track);
  });
}

export function addClip(trackId: string, clip: Omit<Clip, "id">) {
  mutate(() => {
    const track = doc.project.tracks.find((item) => item.id === trackId);
    if (track) track.clips.push({ ...clip, id: newId() });
  });
}

export function deleteSelectedClips() {
  const ids = new Set(doc.selectedClips);
  if (ids.size === 0) return;
  mutate(() => {
    for (const track of doc.project.tracks) {
      track.clips = track.clips.filter((clip) => !ids.has(clip.id));
    }
  });
  doc.selectedClips = [];
}

export function copySelectedClips() {
  const ids = new Set(doc.selectedClips);
  doc.clipboard = allClips()
    .filter((clip) => ids.has(clip.id))
    .map((clip) => plain(clip));
}

export function cutSelectedClips() {
  copySelectedClips();
  deleteSelectedClips();
}

/**
 * Pastes clipboard clips. The target is `trackId`, else the selected track, else the first track.
 * The first pasted clip starts at `time`, or at the playhead when no time is given.
 */
export function pasteClips(options: { time?: number; trackId?: string } = {}): number {
  if (doc.clipboard.length === 0) return 0;
  const target =
    doc.project.tracks.find((track) => track.id === options.trackId) ??
    selectedTrackObject() ??
    doc.project.tracks[0];
  if (!target || !acceptsClips(target.type)) return 0;
  const origin = options.time ?? doc.playhead;
  const first = Math.min(...doc.clipboard.map((clip) => clip.start));
  const created = doc.clipboard.map((clip) => ({
    ...plain(clip),
    id: newId(),
    start: origin + (clip.start - first),
  }));
  mutate(() => {
    target.clips.push(...created);
  });
  doc.selectedTrack = target.id;
  doc.selectedClips = created.map((clip) => clip.id);
  return created.length;
}

/** Duplicates the selected clips right after each original on the same track. */
export function duplicateSelectedClips() {
  const ids = new Set(doc.selectedClips);
  if (ids.size === 0) return;
  const created: string[] = [];
  mutate(() => {
    for (const track of doc.project.tracks) {
      for (const clip of [...track.clips]) {
        if (!ids.has(clip.id)) continue;
        const copy = { ...plain(clip), id: newId(), start: clip.start + clip.duration };
        track.clips.push(copy);
        created.push(copy.id);
      }
    }
  });
  doc.selectedClips = created;
}

export function selectAllClips() {
  doc.selectedClips = allClips().map((clip) => clip.id);
}

export function serializeProject(): string {
  return JSON.stringify(doc.project, null, 2);
}

/** Validates the shape of a loaded project file. Throws with a readable message when it is not a project. */
export function parseProject(text: string): Project {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error("The file is not valid JSON");
  }
  const project = value as Partial<Project>;
  if (project.version !== 1) throw new Error(`Unsupported project version: ${String(project.version)}`);
  if (typeof project.name !== "string" || !Array.isArray(project.tracks)) {
    throw new Error("The project file is missing required fields");
  }
  if (typeof project.tempo !== "number" || project.tempo <= 0) project.tempo = 120;
  if (!Array.isArray(project.timeSignature) || project.timeSignature.length !== 2) project.timeSignature = [4, 4];
  for (const [index, track] of project.tracks.entries()) {
    if (!track || typeof track.id !== "string" || !Array.isArray(track.clips)) {
      throw new Error("The project contains an invalid track");
    }
    track.muted = track.muted === true;
    track.solo = track.solo === true;
    track.height = track.height === "compact" || track.height === "large" ? track.height : "normal";
    if (!isHexColor(track.color)) track.color = TRACK_PALETTE[index % TRACK_PALETTE.length];
    for (const clip of track.clips) {
      clip.path = typeof clip.path === "string" ? clip.path : "";
      if (typeof clip.start !== "number" || typeof clip.duration !== "number") {
        throw new Error("The project contains an invalid clip");
      }
      clip.notes = Array.isArray(clip.notes) ? clip.notes : [];
    }
  }
  return project as Project;
}

export function getActiveClip(): Clip | undefined {
  if (doc.activeClipId) {
    const found = allClips().find((clip) => clip.id === doc.activeClipId);
    if (found) return found;
  }
  if (doc.selectedClips.length > 0) {
    const found = allClips().find((clip) => clip.id === doc.selectedClips[0]);
    if (found) return found;
  }
  // Fall back to first clip of selected track, or first clip in project
  const track = selectedTrackObject();
  if (track && track.clips.length > 0) return track.clips[0];
  const all = allClips();
  return all.length > 0 ? all[0] : undefined;
}

export function addNoteToActiveClip(note: Omit<Note, "id">): Note | null {
  let clip = getActiveClip();
  if (!clip) {
    // If no clip exists, automatically create a clip on the selected track or first voice track!
    const track = selectedTrackObject() ?? doc.project.tracks[0];
    if (!track) return null;
    const newClip: Clip = {
      id: newId(),
      name: "Part 1",
      path: "",
      start: 0,
      duration: 16 * 60 / (doc.project.tempo || 120),
      notes: [],
    };
    mutate(() => {
      track.clips.push(newClip);
      doc.activeClipId = newClip.id;
      doc.selectedClips = [newClip.id];
    });
    clip = newClip;
  }
  const created: Note = { ...note, id: newId() };
  mutate(() => {
    clip!.notes = clip!.notes || [];
    clip!.notes.push(created);
    // ensure clip duration spans at least past this note
    const endSec = (note.start + note.duration) * 60 / (doc.project.tempo || 120);
    if (endSec > clip!.duration) {
      clip!.duration = endSec;
    }
  });
  doc.selectedNote = created.id;
  return created;
}

export function updateNoteInActiveClip(noteId: string, updates: Partial<Note>) {
  const clip = getActiveClip();
  if (!clip || !clip.notes) return;
  const index = clip.notes.findIndex((n) => n.id === noteId);
  if (index === -1) return;
  mutate(() => {
    Object.assign(clip.notes![index], updates);
    const endSec = (clip.notes![index].start + clip.notes![index].duration) * 60 / (doc.project.tempo || 120);
    if (endSec > clip.duration) {
      clip.duration = endSec;
    }
  });
}

export function deleteNoteFromActiveClip(noteId: string) {
  const clip = getActiveClip();
  if (!clip || !clip.notes) return;
  mutate(() => {
    clip.notes = clip.notes!.filter((n) => n.id !== noteId);
    if (doc.selectedNote === noteId) doc.selectedNote = null;
  });
}
