import { save } from "@tauri-apps/plugin-dialog";
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
  await api.importExport.exportDeck(deck.id, path);
  return true;
}
