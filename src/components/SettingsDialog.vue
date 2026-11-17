<script setup lang="ts">
import { reactive, ref } from "vue";
import { LOCALES, t, type MessageKey } from "../i18n";
import {
  COLOR_KEYS,
  PRESETS,
  cloneSettings,
  persistSettings,
  settings,
  type ColorKey,
  type Preset,
} from "../settings";

const emit = defineEmits<{ saved: []; close: [] }>();

const draft = reactive(cloneSettings(settings));
const error = ref("");

const PRESET_IDS: Preset[] = ["concept", "light", "ocean", "custom"];
const PRESET_LABELS: Record<Preset, MessageKey> = {
  concept: "presetConcept",
  light: "presetLight",
  ocean: "presetOcean",
  custom: "presetCustom",
};
const COLOR_LABELS: Record<ColorKey, MessageKey> = {
  background: "colorBackground",
  panel: "colorPanel",
  bar: "colorBar",
  barText: "colorBarText",
  button: "colorButton",
  buttonText: "colorButtonText",
  text: "colorText",
  accent: "colorAccent",
};

function applyPreset(preset: Exclude<Preset, "custom">) {
  draft.theme.preset = preset;
  draft.theme.colors = { ...PRESETS[preset].colors };
}

function setColor(key: ColorKey, value: string) {
  draft.theme.colors[key] = value;
  draft.theme.preset = "custom";
}

async function save() {
  error.value = "";
  try {
    // Start from the latest settings so workspace and recent projects changed while the dialog was open survive.
    const latest = cloneSettings(settings);
    await persistSettings({ ...latest, language: draft.language, theme: cloneSettings(draft).theme });
    emit("saved");
  } catch (e) {
    error.value = t("saveFailed", String(e));
  }
}

function inputValue(event: Event): string {
  return (event.target as HTMLInputElement).value;
}
</script>

<template>
  <div class="overlay" @pointerdown.self="emit('close')">
    <div class="dialog" role="dialog" :aria-label="t('settingsTitle')">
      <div class="dialog-header">
        <span>{{ t("settingsTitle") }}</span>
        <button class="btn small" :title="t('close')" @click="emit('close')">×</button>
      </div>

      <div class="dialog-body">
        <section>
          <h3>{{ t("language") }}</h3>
          <div class="row">
            <button
              v-for="locale in LOCALES"
              :key="locale.code"
              class="btn"
              :class="{ active: draft.language === locale.code }"
              @click="draft.language = locale.code"
            >{{ locale.label }}</button>
          </div>
        </section>

        <section>
          <h3>{{ t("theme") }}</h3>
          <div class="row">
            <span class="label">{{ t("preset") }}</span>
            <button
              v-for="preset in PRESET_IDS"
              :key="preset"
              class="btn"
              :class="{ active: draft.theme.preset === preset }"
              :disabled="preset === 'custom'"
              @click="applyPreset(preset as Exclude<Preset, 'custom'>)"
            >{{ t(PRESET_LABELS[preset]) }}</button>
          </div>
        </section>

        <section>
          <h3>{{ t("colors") }}</h3>
          <div class="grid">
            <label v-for="key in COLOR_KEYS" :key="key" class="color-item">
              <input
                type="color"
                :value="draft.theme.colors[key]"
                @input="setColor(key, inputValue($event))"
              />
              <span>{{ t(COLOR_LABELS[key]) }}</span>
              <code>{{ draft.theme.colors[key] }}</code>
            </label>
          </div>
        </section>

        <p v-if="error" class="error">{{ error }}</p>
      </div>

      <div class="dialog-footer">
        <button class="btn" @click="applyPreset('concept')">{{ t("reset") }}</button>
        <span class="spacer"></span>
        <button class="btn" @click="emit('close')">{{ t("cancel") }}</button>
        <button class="btn active" @click="save">{{ t("save") }}</button>
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
  width: 820px;
  max-height: 880px;
  display: flex;
  flex-direction: column;
  background: var(--panel);
  color: var(--text);
  border: 1px solid var(--accent);
  border-radius: 10px;
  overflow: hidden;
  box-shadow: 0 16px 44px rgba(0, 0, 0, 0.55);
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

.dialog-body {
  display: flex;
  flex-direction: column;
  gap: 18px;
  padding: 18px 20px;
  overflow-y: auto;
}

h3 {
  margin: 0 0 8px;
  font-size: 14px;
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.label {
  opacity: 0.7;
  margin-right: 4px;
}

.grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 10px 20px;
}

.color-item {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
}

.color-item input[type="color"] {
  width: 40px;
  height: 28px;
  padding: 0;
  border: none;
  background: transparent;
  cursor: pointer;
}

.color-item code {
  margin-left: auto;
  opacity: 0.7;
}

.dialog-footer {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 20px;
  border-top: 1px solid var(--button);
}

.spacer {
  flex: 1;
}

.error {
  color: #ff6b6b;
  margin: 0;
}

.btn {
  height: 30px;
  padding: 0 14px;
  border: 2px solid transparent;
  border-radius: 5px;
  background: var(--button);
  color: var(--button-text);
  font: 500 13px Inter, Avenir, Helvetica, Arial, sans-serif;
  cursor: pointer;
}

.btn.small {
  height: 26px;
  padding: 0 8px;
}

.btn.active {
  border-color: var(--accent);
}

.btn:disabled {
  opacity: 0.5;
  cursor: default;
}
</style>
