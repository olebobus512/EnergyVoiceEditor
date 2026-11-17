import { reactive } from "vue";
import type { MenuItem } from "./components/MenuBar.vue";

/** Shared state of the one right-click menu. Whoever handles `contextmenu` builds the items and calls showContextMenu. */
export const contextMenu = reactive({
  open: false,
  x: 0,
  y: 0,
  items: [] as MenuItem[],
  /** Changes on every open so the component resets its focus state even when reopened at the same spot. */
  token: 0,
});

export function showContextMenu(x: number, y: number, items: MenuItem[]) {
  contextMenu.x = x;
  contextMenu.y = y;
  contextMenu.items = items;
  contextMenu.token += 1;
  contextMenu.open = items.length > 0;
}

export function hideContextMenu() {
  contextMenu.open = false;
}

/** What was right-clicked in the arrangement. App.vue turns this into menu items. */
export type ArrangementContext =
  | { kind: "track"; trackId: string }
  | { kind: "clip"; trackId: string; clipId: string }
  | { kind: "lane"; trackId: string; time: number }
  | { kind: "ruler"; time: number }
  | { kind: "empty"; time: number };
