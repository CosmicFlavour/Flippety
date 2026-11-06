import { invoke } from "@tauri-apps/api/core";
import type {
  Card,
  CardPage,
  Deck,
  Direction,
  DueItem,
  ImportMode,
  NewCard,
  NewDeck,
  SubmitReviewInput,
  UpdateCard,
  UpdateDeck,
} from "@/types/models";

// Thin, typed passthrough to Tauri commands. No logic lives here — every
// decision (validation, scheduling, persistence) happens in Rust.
export const api = {
  decks: {
    list: () => invoke<Deck[]>("list_decks"),
    create: (input: NewDeck) => invoke<Deck>("create_deck", { input }),
    rename: (input: UpdateDeck) => invoke<void>("rename_deck", { input }),
    delete: (id: string) => invoke<void>("delete_deck", { id }),
  },
  cards: {
    listByDeck: (
      deckId: string,
      options?: { search?: string; tags?: string[]; limit?: number; offset?: number },
    ) =>
      invoke<CardPage>("list_cards", {
        deckId,
        search: options?.search,
        tags: options?.tags,
        limit: options?.limit,
        offset: options?.offset,
      }),
    listDeckTags: (deckId: string) => invoke<string[]>("list_deck_tags", { deckId }),
    create: (input: NewCard) => invoke<Card>("create_card", { input }),
    update: (input: UpdateCard) => invoke<Card>("update_card", { input }),
    delete: (id: string) => invoke<void>("delete_card", { id }),
    resetProgress: (id: string) => invoke<void>("reset_card_progress", { id }),
  },
  study: {
    nextItem: (deckId: string | null, exclude?: { card_id: string; direction: Direction }) =>
      invoke<DueItem | null>("get_next_study_item", { deckId, exclude }),
    submitReview: (input: SubmitReviewInput) => invoke<void>("submit_review", { input }),
  },
  importExport: {
    // Returns/takes file *content*, not a path — writing/reading the file
    // itself is the caller's job (via the fs plugin), since on Android the
    // dialog plugin hands back a `content://` URI rather than a real path.
    exportDeck: (deckId: string) => invoke<string>("export_deck", { deckId }),
    importDeck: (content: string, mode: ImportMode) =>
      invoke<Deck>("import_deck", { content, mode }),
  },
};
