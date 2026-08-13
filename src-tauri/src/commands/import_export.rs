use super::AppState;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::card::{default_directions, default_level, Card, CardFull, Direction};
use crate::models::deck::Deck;
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::fs;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
struct DeckMeta {
    name: String,
    description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CardExport {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    face_1: String,
    face_2: String,
    full: CardFull,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default = "default_directions")]
    directions: Vec<Direction>,
    #[serde(default = "default_level")]
    level: i64,
}

#[derive(Debug, Serialize)]
struct DeckExport {
    deck: DeckMeta,
    cards: Vec<CardExport>,
}

#[derive(Debug, Deserialize)]
struct DeckImport {
    deck: DeckMeta,
    cards: Vec<CardExport>,
}

fn export_deck_json(conn: &Connection, deck_id: &str) -> AppResult<String> {
    let deck = db::decks::get(conn, deck_id)?
        .ok_or_else(|| AppError::NotFound(format!("deck {deck_id}")))?;
    let cards = db::cards::list_by_deck(conn, deck_id)?;

    let export = DeckExport {
        deck: DeckMeta {
            name: deck.name,
            description: deck.description,
        },
        cards: cards
            .into_iter()
            .map(|c| CardExport {
                id: Some(c.id),
                face_1: c.face_1,
                face_2: c.face_2,
                full: c.full,
                tags: c.tags,
                directions: c.directions,
                level: c.level,
            })
            .collect(),
    };

    Ok(serde_json::to_string_pretty(&export)?)
}

/// Writes a deck and its cards' content (no review progress) to `target_path` as JSON.
#[tauri::command]
pub fn export_deck(state: State<AppState>, deck_id: String, target_path: String) -> AppResult<()> {
    let conn = state.conn();
    let json = export_deck_json(&conn, &deck_id)?;
    fs::write(target_path, json)?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ImportMode {
    New,
    Merge { deck_id: String },
}

/// Imports a deck from `raw` JSON. Cards whose `id` already exists (only
/// possible in Merge mode, re-importing a deck you already have) get their
/// content updated in place, and any review progress on unchanged directions
/// is left untouched. New directions on an existing card get fresh FSRS state.
///
/// Runs as a single transaction so a mid-import failure (bad row, I/O error,
/// crash) leaves the database exactly as it was, never a half-imported deck.
fn import_deck_json(conn: &mut Connection, raw: &str, mode: ImportMode) -> AppResult<Deck> {
    let parsed: DeckImport = serde_json::from_str(raw)?;

    for card in &parsed.cards {
        if card.face_1.trim().is_empty() || card.face_2.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "every card needs both face_1 and face_2".into(),
            ));
        }
        if card.directions.is_empty() {
            return Err(AppError::InvalidInput(
                "every card needs at least one direction".into(),
            ));
        }
    }

    let now = Utc::now();
    let tx = conn.transaction()?;

    let deck = match mode {
        ImportMode::New => {
            let name = parsed.deck.name.trim().to_string();
            if name.is_empty() {
                return Err(AppError::InvalidInput("deck name cannot be empty".into()));
            }
            let deck = Deck {
                id: Uuid::new_v4().to_string(),
                name,
                description: parsed.deck.description,
                new_cards_per_day: None,
                created_at: now,
                updated_at: now,
            };
            db::decks::insert(&tx, &deck)?;
            deck
        }
        ImportMode::Merge { deck_id } => db::decks::get(&tx, &deck_id)?
            .ok_or_else(|| AppError::NotFound(format!("deck {deck_id}")))?,
    };

    for card_export in parsed.cards {
        let existing = match &card_export.id {
            Some(id) => db::cards::get(&tx, id)?,
            None => None,
        };

        let card = Card {
            id: existing
                .as_ref()
                .map(|c| c.id.clone())
                .or(card_export.id)
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            deck_id: deck.id.clone(),
            face_1: card_export.face_1,
            face_2: card_export.face_2,
            full: card_export.full,
            tags: card_export.tags,
            directions: card_export.directions,
            level: card_export.level,
            created_at: existing.as_ref().map(|c| c.created_at).unwrap_or(now),
            updated_at: now,
        };

        if existing.is_some() {
            db::cards::update(&tx, &card)?;
        } else {
            db::cards::insert(&tx, &card)?;
        }

        db::review_state::seed_missing(&tx, &card.id, &card.directions, now)?;
    }

    tx.commit()?;

    Ok(deck)
}

#[tauri::command]
pub fn import_deck(
    state: State<AppState>,
    source_path: String,
    mode: ImportMode,
) -> AppResult<Deck> {
    let raw = fs::read_to_string(&source_path)?;
    let mut conn = state.conn();
    import_deck_json(&mut conn, &raw, mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::decks::tests_support::new_deck;

    fn sample_export_json(deck_name: &str, card_id: Option<&str>) -> String {
        format!(
            r#"{{
                "deck": {{ "name": "{deck_name}", "description": "Imported deck" }},
                "cards": [
                    {{
                        {id_field}
                        "face_1": "dog",
                        "face_2": "狗",
                        "full": {{ "title": "狗", "subtitle": "gǒu", "body": "Domestic dog.", "foot": "" }},
                        "tags": ["animals"],
                        "directions": ["1->2", "2->1"]
                    }}
                ]
            }}"#,
            id_field = card_id
                .map(|id| format!("\"id\": \"{id}\","))
                .unwrap_or_default(),
        )
    }

    #[test]
    fn export_deck_json_includes_deck_meta_and_all_card_content() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Chinese HSK1");
        let card = Card {
            id: Uuid::new_v4().to_string(),
            deck_id: deck.id.clone(),
            face_1: "dog".into(),
            face_2: "狗".into(),
            full: CardFull {
                title: "狗".into(),
                subtitle: "gǒu".into(),
                body: "Domestic dog.".into(),
                foot: String::new(),
            },
            tags: vec!["animals".into()],
            directions: Direction::all().to_vec(),
            level: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db::cards::insert(&conn, &card).unwrap();

        let json = export_deck_json(&conn, &deck.id).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["deck"]["name"], "Chinese HSK1");
        assert_eq!(value["cards"][0]["id"], card.id);
        assert_eq!(value["cards"][0]["face_1"], "dog");
        assert_eq!(value["cards"][0]["face_2"], "狗");
        assert_eq!(value["cards"][0]["tags"][0], "animals");
    }

    #[test]
    fn export_deck_json_does_not_leak_review_progress_fields() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Chinese HSK1");
        let card = Card {
            id: Uuid::new_v4().to_string(),
            deck_id: deck.id.clone(),
            face_1: "dog".into(),
            face_2: "狗".into(),
            full: CardFull {
                title: "狗".into(),
                subtitle: String::new(),
                body: String::new(),
                foot: String::new(),
            },
            tags: vec![],
            directions: Direction::all().to_vec(),
            level: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db::cards::insert(&conn, &card).unwrap();

        let json = export_deck_json(&conn, &deck.id).unwrap();

        assert!(!json.contains("stability"));
        assert!(!json.contains("due"));
        assert!(!json.contains("reps"));
    }

    #[test]
    fn export_deck_returns_not_found_for_a_missing_deck() {
        let conn = db::test_connection();
        let err = export_deck_json(&conn, "does-not-exist").unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn import_deck_rejects_malformed_json() {
        let mut conn = db::test_connection();
        let err = import_deck_json(&mut conn, "{ not valid json", ImportMode::New).unwrap_err();
        assert!(matches!(err, AppError::Json(_)));
    }

    #[test]
    fn import_deck_new_creates_deck_and_cards_with_seeded_review_state() {
        let mut conn = db::test_connection();
        let json = sample_export_json("Imported Deck", None);

        let deck = import_deck_json(&mut conn, &json, ImportMode::New).unwrap();

        assert_eq!(deck.name, "Imported Deck");
        let cards = db::cards::list_by_deck(&conn, &deck.id).unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].face_1, "dog");
        assert!(
            db::review_state::get(&conn, &cards[0].id, Direction::FaceOneToTwo)
                .unwrap()
                .is_some()
        );
        assert!(
            db::review_state::get(&conn, &cards[0].id, Direction::FaceTwoToOne)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn import_deck_new_rejects_an_empty_deck_name() {
        let mut conn = db::test_connection();
        let json = sample_export_json("   ", None);

        let err = import_deck_json(&mut conn, &json, ImportMode::New).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn import_deck_rejects_a_card_with_an_empty_face() {
        let mut conn = db::test_connection();
        let json = r#"{
            "deck": { "name": "Deck", "description": null },
            "cards": [
                { "face_1": "", "face_2": "狗", "full": { "title": "", "subtitle": "", "body": "", "foot": "" }, "directions": ["1->2"] }
            ]
        }"#;

        let err = import_deck_json(&mut conn, json, ImportMode::New).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn import_deck_rejects_a_card_with_no_directions() {
        let mut conn = db::test_connection();
        let json = r#"{
            "deck": { "name": "Deck", "description": null },
            "cards": [
                { "face_1": "dog", "face_2": "狗", "full": { "title": "", "subtitle": "", "body": "", "foot": "" }, "directions": [] }
            ]
        }"#;

        let err = import_deck_json(&mut conn, json, ImportMode::New).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn import_deck_merge_adds_cards_to_an_existing_deck() {
        let mut conn = db::test_connection();
        let deck = new_deck(&conn, "Existing deck");
        let json = sample_export_json("Ignored on merge", None);

        let result = import_deck_json(
            &mut conn,
            &json,
            ImportMode::Merge {
                deck_id: deck.id.clone(),
            },
        )
        .unwrap();

        assert_eq!(result.id, deck.id);
        assert_eq!(result.name, "Existing deck");
        assert_eq!(db::cards::list_by_deck(&conn, &deck.id).unwrap().len(), 1);
    }

    #[test]
    fn import_deck_merge_returns_not_found_for_a_missing_deck() {
        let mut conn = db::test_connection();
        let json = sample_export_json("Deck", None);

        let err = import_deck_json(
            &mut conn,
            &json,
            ImportMode::Merge {
                deck_id: "does-not-exist".into(),
            },
        )
        .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn import_deck_merge_updates_an_existing_cards_content_by_id_without_resetting_its_progress() {
        let mut conn = db::test_connection();
        let deck = new_deck(&conn, "Existing deck");
        let card = Card {
            id: Uuid::new_v4().to_string(),
            deck_id: deck.id.clone(),
            face_1: "dog".into(),
            face_2: "狗".into(),
            full: CardFull {
                title: "狗".into(),
                subtitle: "gǒu".into(),
                body: String::new(),
                foot: String::new(),
            },
            tags: vec![],
            directions: Direction::all().to_vec(),
            level: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db::cards::insert(&conn, &card).unwrap();
        let fsrs_state = crate::srs::new_card_state(Utc::now());
        db::review_state::seed(&conn, &card.id, Direction::FaceOneToTwo, &fsrs_state).unwrap();
        db::review_state::seed(&conn, &card.id, Direction::FaceTwoToOne, &fsrs_state).unwrap();

        // Simulate review progress before the re-import.
        let mut reviewed = db::review_state::get(&conn, &card.id, Direction::FaceOneToTwo)
            .unwrap()
            .unwrap();
        reviewed.reps = 9;
        db::review_state::upsert(&conn, &card.id, Direction::FaceOneToTwo, &reviewed).unwrap();

        let json = sample_export_json("Existing deck", Some(&card.id))
            .replace("\"dog\"", "\"dog (updated)\"");

        import_deck_json(
            &mut conn,
            &json,
            ImportMode::Merge {
                deck_id: deck.id.clone(),
            },
        )
        .unwrap();

        let reloaded = db::cards::get(&conn, &card.id).unwrap().unwrap();
        assert_eq!(reloaded.face_1, "dog (updated)");
        let progress = db::review_state::get(&conn, &card.id, Direction::FaceOneToTwo)
            .unwrap()
            .unwrap();
        assert_eq!(progress.reps, 9);
    }

    #[test]
    fn import_deck_merge_seeds_a_newly_added_direction_on_an_existing_card() {
        let mut conn = db::test_connection();
        let deck = new_deck(&conn, "Existing deck");
        let card = Card {
            id: Uuid::new_v4().to_string(),
            deck_id: deck.id.clone(),
            face_1: "dog".into(),
            face_2: "狗".into(),
            full: CardFull {
                title: "狗".into(),
                subtitle: String::new(),
                body: String::new(),
                foot: String::new(),
            },
            tags: vec![],
            directions: vec![Direction::FaceOneToTwo],
            level: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db::cards::insert(&conn, &card).unwrap();
        let fsrs_state = crate::srs::new_card_state(Utc::now());
        db::review_state::seed(&conn, &card.id, Direction::FaceOneToTwo, &fsrs_state).unwrap();

        let json = sample_export_json("Existing deck", Some(&card.id));

        import_deck_json(
            &mut conn,
            &json,
            ImportMode::Merge {
                deck_id: deck.id.clone(),
            },
        )
        .unwrap();

        assert!(
            db::review_state::get(&conn, &card.id, Direction::FaceTwoToOne)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn export_then_import_round_trip_preserves_card_ids_and_content() {
        // Export from one instance (e.g. a backup)...
        let source_conn = db::test_connection();
        let deck = new_deck(&source_conn, "Chinese HSK1");
        let card = Card {
            id: Uuid::new_v4().to_string(),
            deck_id: deck.id.clone(),
            face_1: "dog".into(),
            face_2: "狗".into(),
            full: CardFull {
                title: "狗".into(),
                subtitle: "gǒu".into(),
                body: "Domestic dog.".into(),
                foot: String::new(),
            },
            tags: vec!["animals".into()],
            directions: Direction::all().to_vec(),
            level: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db::cards::insert(&source_conn, &card).unwrap();
        let json = export_deck_json(&source_conn, &deck.id).unwrap();

        // ...and import into a fresh instance (e.g. a restore, or sharing the deck).
        let mut target_conn = db::test_connection();
        let reimported = import_deck_json(&mut target_conn, &json, ImportMode::New).unwrap();

        let cards = db::cards::list_by_deck(&target_conn, &reimported.id).unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].id, card.id);
        assert_eq!(cards[0].face_1, "dog");
        assert_eq!(cards[0].full.subtitle, "gǒu");
    }
}
