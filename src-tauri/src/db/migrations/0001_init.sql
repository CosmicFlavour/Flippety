CREATE TABLE decks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE cards (
    id TEXT PRIMARY KEY,
    deck_id TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    face_1 TEXT NOT NULL,
    face_2 TEXT NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    subtitle TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    foot TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '[]',
    directions TEXT NOT NULL DEFAULT '["1->2","2->1"]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_cards_deck_id ON cards(deck_id);

CREATE TABLE review_state (
    card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    direction TEXT NOT NULL CHECK (direction IN ('1->2', '2->1')),
    due TEXT NOT NULL,
    stability REAL NOT NULL,
    difficulty REAL NOT NULL,
    elapsed_days INTEGER NOT NULL,
    scheduled_days INTEGER NOT NULL,
    reps INTEGER NOT NULL,
    lapses INTEGER NOT NULL,
    state TEXT NOT NULL,
    last_review TEXT NOT NULL,
    PRIMARY KEY (card_id, direction)
);
CREATE INDEX idx_review_state_due ON review_state(due);

CREATE TABLE review_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    direction TEXT NOT NULL CHECK (direction IN ('1->2', '2->1')),
    rating TEXT NOT NULL,
    elapsed_days INTEGER NOT NULL,
    scheduled_days INTEGER NOT NULL,
    state TEXT NOT NULL,
    reviewed_at TEXT NOT NULL
);
CREATE INDEX idx_review_log_card_id ON review_log(card_id);
