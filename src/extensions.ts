import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
export interface InstrumentPlugin { id: string; name: string; version: number; harmonics: number[]; attack: number; release: number; decay: number }
export const instrumentPlugins = ref<InstrumentPlugin[]>([]);
export async function refreshInstruments() { instrumentPlugins.value = await invoke<InstrumentPlugin[]>("list_instruments"); }
