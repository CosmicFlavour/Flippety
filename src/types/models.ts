// Mirrors src-tauri/src/models. Field names here are wire format (serde
// defaults, snake_case) since Tauri does not re-case fields inside a struct
// body — only top-level command argument names get camelCase treatment.

export type Direction = "1->2" | "2->1";

export interface CardFull {
  title: string;
  subtitle: string;
  body: string;
  foot: string;
}

export interface Deck {
  id: string;
  name: string;
  description: string | null;
  /** Cap on how many new cards may be introduced per day. `null` = unlimited. */
  new_cards_per_day: number | null;
  created_at: string;
  updated_at: string;
}

export interface Card {
  id: string;
  deck_id: string;
  face_1: string;
  face_2: string;
  full: CardFull;
  tags: string[];
  directions: Direction[];
  level: number;
  created_at: string;
  updated_at: string;
}

export interface CardPage {
  items: Card[];
  /** Total rows matching the filter, ignoring limit/offset. */
  total: number;
}

export interface NewDeck {
  name: string;
  description?: string | null;
  new_cards_per_day?: number | null;
}

export interface UpdateDeck {
  id: string;
  name: string;
  description?: string | null;
  new_cards_per_day?: number | null;
}

export interface NewCard {
  deck_id: string;
  face_1: string;
  face_2: string;
  full: CardFull;
  tags?: string[];
  directions?: Direction[];
  level?: number;
}

export interface UpdateCard {
  id: string;
  face_1: string;
  face_2: string;
  full: CardFull;
  tags: string[];
  directions: Direction[];
  level: number;
}

export type Rating = "Again" | "Hard" | "Good" | "Easy";

export interface DueItem {
  card_id: string;
  direction: Direction;
  prompt: string;
  full: CardFull;
}

/** A lightweight (card_id, direction) reference into the queue, with no card content. */
export interface QueueRef {
  card_id: string;
  direction: Direction;
}

export interface QueueManifest {
  due: QueueRef[];
  new: QueueRef[];
}

export interface SubmitReviewInput {
  card_id: string;
  direction: Direction;
  rating: Rating;
}

export type ImportMode = { mode: "new" } | { mode: "merge"; deck_id: string };
