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
  /** Which direction of a new two-direction card is introduced first; the
   * other direction's introduction is staggered. Moot for single-direction
   * cards. */
  intro_direction: Direction;
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
  intro_direction?: Direction;
}

export interface UpdateDeck {
  id: string;
  name: string;
  description?: string | null;
  intro_direction?: Direction;
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

export interface SubmitReviewInput {
  card_id: string;
  direction: Direction;
  rating: Rating;
}

export interface SubmitReviewResult {
  /** Present only when this review was a card's first in this direction and
   * the deck's local-day new-card count landed on a multiple of 5. */
  milestone: number | null;
}

export type ImportMode = { mode: "new" } | { mode: "merge"; deck_id: string };
