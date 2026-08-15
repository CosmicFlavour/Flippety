import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { api } from "@/lib/api";
import type { Deck } from "@/types/models";

/** Opens a native save dialog and exports the deck there. Resolves to false if the user cancels. */
export async function exportDeckToFile(deck: Deck): Promise<boolean> {
  const path = await save({
    title: `Export "${deck.name}"`,
    defaultPath: `${deck.name}.json`,
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (!path) return false;
  const json = await api.importExport.exportDeck(deck.id);
  // Writing via the fs plugin (rather than a Rust-side fs::write) is what
  // makes this work on Android, where `path` here is a `content://` URI, not
  // a real filesystem path.
  await writeTextFile(path, json);
  return true;
}
