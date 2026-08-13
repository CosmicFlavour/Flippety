ALTER TABLE cards ADD COLUMN level INTEGER NOT NULL DEFAULT 1;
CREATE INDEX idx_cards_deck_id_level ON cards(deck_id, level);
ALTER TABLE decks ADD COLUMN new_cards_per_day INTEGER;
