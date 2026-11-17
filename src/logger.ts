import { reactive } from "vue";

export type LogLevel = "INFO" | "WARNING" | "ERROR" | "DEBUG";

export interface LogEntry {
  id: number;
  time: string;
  level: LogLevel;
  source: string;
  message: string;
}

const MAX_ENTRIES = 2000;
let nextId = 1;

export const logStore = reactive({ entries: [] as LogEntry[] });

/** Adds a real application or engine event to the console. Never call with made-up messages. */
export function log(level: LogLevel, source: string, message: string) {
  logStore.entries.push({
    id: nextId++,
    time: new Date().toLocaleTimeString(),
    level,
    source,
    message,
  });
  if (logStore.entries.length > MAX_ENTRIES) logStore.entries.shift();
}

export const logInfo = (message: string, source = "app") => log("INFO", source, message);
export const logWarning = (message: string, source = "app") => log("WARNING", source, message);
export const logError = (message: string, source = "app") => log("ERROR", source, message);
export const logDebug = (message: string, source = "app") => log("DEBUG", source, message);

export function clearLog() {
  logStore.entries = [];
}
