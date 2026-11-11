ALTER TABLE decks DROP COLUMN new_cards_per_day;
ALTER TABLE decks ADD COLUMN intro_direction TEXT NOT NULL DEFAULT '1->2' CHECK (intro_direction IN ('1->2', '2->1'));
