// Per-track colours. Each track stores its own colour in the project file; there is no global track colour setting.
// Default colours for new tracks.

export const TRACK_PALETTE = [
  "#ff5c7a", // Track 1 Coral
  "#3ddc97", // Track 2 Green
  "#5b8cff", // Track 3 Blue
  "#ffd166", // Yellow accent
  "#2de2c4", // Teal
  "#b14bff", // Purple
  "#ff9500", // Orange
  "#ff4fa3", // Pink
  "#00d4ff", // Cyan
  "#a3e635", // Lime
  "#c27c4e", // Brown
  "#9aa3b8", // Slate
];

export function isHexColor(value: unknown): value is string {
  return typeof value === "string" && /^#[0-9a-fA-F]{6}$/.test(value);
}

/** Picks the palette colour used by the fewest tracks; ties go to the earliest palette entry. */
export function pickDefaultColor(used: string[]): string {
  const counts = TRACK_PALETTE.map((color) => used.filter((item) => item.toLowerCase() === color.toLowerCase()).length);
  return TRACK_PALETTE[counts.indexOf(Math.min(...counts))];
}

/** Black or white, whichever is easier to read on top of the given colour. */
export function contrastText(hex: string): string {
  if (!isHexColor(hex)) return "#000000";
  const channel = (offset: number) => {
    const value = parseInt(hex.slice(offset, offset + 2), 16) / 255;
    return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  const luminance = 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
  return luminance > 0.35 ? "#000000" : "#FFFFFF";
}
