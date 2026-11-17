import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { Locale } from "./i18n";

export const COLOR_KEYS = [
  "background",
  "panel",
  "bar",
  "barText",
  "button",
  "buttonText",
  "text",
  "accent",
] as const;

export type ColorKey = (typeof COLOR_KEYS)[number];
export type Preset = "concept" | "light" | "ocean" | "custom";

export interface Theme {
  preset: Preset;
  colors: Record<ColorKey, string>;
}

export interface Workspace {
  trackPanel: boolean;
  pianoRoll: boolean;
  terminal: boolean;
  pxPerSec: number;
  trackPanelWidth: number;
  bottomHeight: number;
  terminalWidth: number;
}

export interface Settings {
  language: Locale;
  theme: Theme;
  workspace: Workspace;
  recentProjects: string[];
}

export const DEFAULT_WORKSPACE: Workspace = {
  trackPanel: true,
  pianoRoll: true,
  terminal: true,
  pxPerSec: 60,
  trackPanelWidth: 300,
  bottomHeight: 420,
  terminalWidth: 511,
};

type PresetDefinition = { colors: Record<ColorKey, string> };

/** The "concept" preset is the colour scheme of UI-Concept.figcraft. Keep it in sync with settings.rs. */
export const PRESETS: Record<Exclude<Preset, "custom">, PresetDefinition> = {
  concept: {
    colors: {
      background: "#101010", panel: "#181818", bar: "#252525", barText: "#EEEEEE",
      button: "#292929", buttonText: "#EEEEEE", text: "#EEEEEE", accent: "#F2CD65",
    },
  },
  light: {
    colors: {
      background: "#E6E6E6",
      panel: "#FFFFFF",
      bar: "#F5F5F5",
      barText: "#111111",
      button: "#DCDCDC",
      buttonText: "#111111",
      text: "#111111",
      accent: "#1565C0",
    },
  },
  ocean: {
    colors: {
      background: "#07121F",
      panel: "#0E2236",
      bar: "#1B3A5C",
      barText: "#E6F1FF",
      button: "#2A5079",
      buttonText: "#E6F1FF",
      text: "#E6F1FF",
      accent: "#5BC0EB",
    },
  },
};

export const settings = reactive<Settings>({
  language: "en",
  theme: { preset: "concept", ...structuredClone(PRESETS.concept) },
  workspace: { ...DEFAULT_WORKSPACE },
  recentProjects: [],
});

/** Plain copy; structuredClone cannot clone Vue reactive proxies. */
export function cloneSettings(source: Settings): Settings {
  return JSON.parse(JSON.stringify(source));
}

/** Updates in place so components holding references to settings.workspace keep working. */
function replaceSettings(next: Settings) {
  settings.language = next.language;
  settings.theme = next.theme;
  Object.assign(settings.workspace, next.workspace);
  settings.recentProjects = next.recentProjects;
}

/** Writes the theme into CSS variables (--background, --bar-text, ...) and the document language. Track colours are per track, not part of the theme. */
export function applySettings(source: Settings) {
  const root = document.documentElement;
  for (const key of COLOR_KEYS) {
    const name = key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
    root.style.setProperty(`--${name}`, source.theme.colors[key]);
  }
  root.lang = source.language;
}

/** Loads settings from the Rust core. Returns an error message on failure; defaults stay in place. */
export async function loadSettings(): Promise<string | null> {
  try {
    replaceSettings(await invoke<Settings>("get_settings"));
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Validates and saves settings through the Rust core. Throws the Rust error message on failure. */
export async function persistSettings(next: Settings): Promise<void> {
  replaceSettings(await invoke<Settings>("save_settings", { settings: next }));
}
