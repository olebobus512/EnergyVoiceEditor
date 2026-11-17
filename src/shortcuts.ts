// Central keyboard shortcut table. Menus read their shortcut labels from here, so the two never disagree.

export const SHORTCUTS = {
  newProject: "Ctrl+N",
  openProject: "Ctrl+O",
  save: "Ctrl+S",
  saveAs: "Ctrl+Shift+S",
  undo: "Ctrl+Z",
  redo: "Ctrl+Y",
  cut: "Ctrl+X",
  copy: "Ctrl+C",
  paste: "Ctrl+V",
  duplicate: "Ctrl+D",
  delete: "Delete",
  selectAll: "Ctrl+A",
  playPause: "Space",
  escape: "Escape",
} as const;

export type ActionId = keyof typeof SHORTCUTS;

/** Shortcuts that also work while a text field has focus. Editing shortcuts stay native inside text fields. */
const WORKS_IN_TEXT_FIELDS: ActionId[] = ["newProject", "openProject", "save", "saveAs", "escape"];

export function isTextInput(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  if (target instanceof HTMLTextAreaElement) return true;
  if (target instanceof HTMLInputElement) {
    return !["range", "color", "checkbox", "radio", "button"].includes(target.type);
  }
  return false;
}

function matches(event: KeyboardEvent, combo: string): boolean {
  const parts = combo.split("+");
  const key = parts[parts.length - 1];
  const wantsCtrl = parts.includes("Ctrl");
  const wantsShift = parts.includes("Shift");
  if (event.ctrlKey !== wantsCtrl || event.shiftKey !== wantsShift || event.altKey) return false;
  if (key === "Space") return event.key === " ";
  return event.key.toLowerCase() === key.toLowerCase();
}

/** Calls `run` for the first shortcut that matches the event. Returns true if one matched. */
export function dispatchShortcut(event: KeyboardEvent, run: (id: ActionId) => void): boolean {
  const inText = isTextInput(event.target);
  for (const id of Object.keys(SHORTCUTS) as ActionId[]) {
    if (!matches(event, SHORTCUTS[id])) continue;
    if (inText && !WORKS_IN_TEXT_FIELDS.includes(id)) return false;
    event.preventDefault();
    run(id);
    return true;
  }
  return false;
}
