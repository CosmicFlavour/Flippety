import { invoke } from "@tauri-apps/api/core";
import type {
  Card,
  CardPage,
  Deck,
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
    create: (input: NewCard) => invoke<Card>("create_card", { input }),
    update: (input: UpdateCard) => invoke<Card>("update_card", { input }),
    delete: (id: string) => invoke<void>("delete_card", { id }),
    resetProgress: (id: string) => invoke<void>("reset_card_progress", { id }),
  },
  study: {
    dueQueue: (deckId: string | null, limit: number) =>
      invoke<DueItem[]>("get_due_queue", { deckId, limit }),
    submitReview: (input: SubmitReviewInput) => invoke<void>("submit_review", { input }),
  },
  importExport: {
    exportDeck: (deckId: string, targetPath: string) =>
      invoke<void>("export_deck", { deckId, targetPath }),
    importDeck: (sourcePath: string, mode: ImportMode) =>
      invoke<Deck>("import_deck", { sourcePath, mode }),
  },
};
