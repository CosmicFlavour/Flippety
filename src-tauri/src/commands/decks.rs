use super::AppState;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::deck::{Deck, NewDeck, UpdateDeck};
use chrono::Utc;
use rusqlite::Connection;
use tauri::State;
use uuid::Uuid;

fn create_deck_inner(conn: &Connection, input: NewDeck) -> AppResult<Deck> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::InvalidInput("deck name cannot be empty".into()));
    }

    let now = Utc::now();
    let deck = Deck {
        id: Uuid::new_v4().to_string(),
        name,
        description: input.description,
        new_cards_per_day: input.new_cards_per_day,
        created_at: now,
        updated_at: now,
    };
    db::decks::insert(conn, &deck)?;
    Ok(deck)
}

fn rename_deck_inner(conn: &Connection, input: UpdateDeck) -> AppResult<()> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::InvalidInput("deck name cannot be empty".into()));
    }

    let existing = db::decks::get(conn, &input.id)?
        .ok_or_else(|| AppError::NotFound(format!("deck {}", input.id)))?;
    let updated = Deck {
        name,
        description: input.description,
        new_cards_per_day: input.new_cards_per_day,
        updated_at: Utc::now(),
        ..existing
    };
    db::decks::update(conn, &updated)?;
    Ok(())
}

fn delete_deck_inner(conn: &Connection, id: &str) -> AppResult<()> {
    if !db::decks::delete(conn, id)? {
        return Err(AppError::NotFound(format!("deck {id}")));
    }
    Ok(())
}

#[tauri::command]
pub fn list_decks(state: State<AppState>) -> AppResult<Vec<Deck>> {
    let conn = state.conn();
    db::decks::list(&conn)
}

#[tauri::command]
pub fn create_deck(state: State<AppState>, input: NewDeck) -> AppResult<Deck> {
    let conn = state.conn();
    create_deck_inner(&conn, input)
}

#[tauri::command]
pub fn rename_deck(state: State<AppState>, input: UpdateDeck) -> AppResult<()> {
    let conn = state.conn();
    rename_deck_inner(&conn, input)
}

#[tauri::command]
pub fn delete_deck(state: State<AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    delete_deck_inner(&conn, &id)
}

/// Shared by other modules' tests that just need *a* deck to hang cards off of.
#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;

    pub fn new_deck(conn: &Connection, name: &str) -> Deck {
        create_deck_inner(
            conn,
            NewDeck {
                name: name.to_string(),
                description: None,
                new_cards_per_day: None,
            },
        )
        .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::card::{Card, CardFull, Direction};

    fn new_deck_input(name: &str) -> NewDeck {
        NewDeck {
            name: name.to_string(),
            description: Some("A test deck".into()),
            new_cards_per_day: None,
        }
    }

    #[test]
    fn create_deck_trims_the_name_and_persists_it() {
        let conn = db::test_connection();

        let deck = create_deck_inner(&conn, new_deck_input("  Chinese HSK1  ")).unwrap();

        assert_eq!(deck.name, "Chinese HSK1");
        let reloaded = db::decks::get(&conn, &deck.id).unwrap().unwrap();
        assert_eq!(reloaded.name, "Chinese HSK1");
        assert_eq!(reloaded.description.as_deref(), Some("A test deck"));
    }

    #[test]
    fn create_deck_rejects_an_empty_or_whitespace_name() {
        let conn = db::test_connection();

        let err = create_deck_inner(&conn, new_deck_input("   ")).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn rename_deck_persists_the_new_name_and_description() {
        let conn = db::test_connection();
        let deck = create_deck_inner(&conn, new_deck_input("Chinese HSK1")).unwrap();

        rename_deck_inner(
            &conn,
            UpdateDeck {
                id: deck.id.clone(),
                name: "  Chinese HSK1 (renamed)  ".into(),
                description: Some("Updated description".into()),
                new_cards_per_day: None,
            },
        )
        .unwrap();

        let reloaded = db::decks::get(&conn, &deck.id).unwrap().unwrap();
        assert_eq!(reloaded.name, "Chinese HSK1 (renamed)");
        assert_eq!(reloaded.description.as_deref(), Some("Updated description"));
    }

    #[test]
    fn rename_deck_rejects_an_empty_name() {
        let conn = db::test_connection();
        let deck = create_deck_inner(&conn, new_deck_input("Chinese HSK1")).unwrap();

        let err = rename_deck_inner(
            &conn,
            UpdateDeck {
                id: deck.id,
                name: "   ".into(),
                description: None,
                new_cards_per_day: None,
            },
        )
        .unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn rename_deck_returns_not_found_for_a_missing_id() {
        let conn = db::test_connection();

        let err = rename_deck_inner(
            &conn,
            UpdateDeck {
                id: "does-not-exist".into(),
                name: "Anything".into(),
                description: None,
                new_cards_per_day: None,
            },
        )
        .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn delete_deck_removes_it() {
        let conn = db::test_connection();
        let deck = create_deck_inner(&conn, new_deck_input("Chinese HSK1")).unwrap();

        delete_deck_inner(&conn, &deck.id).unwrap();

        assert!(db::decks::get(&conn, &deck.id).unwrap().is_none());
    }

    #[test]
    fn delete_deck_returns_not_found_for_a_missing_id() {
        let conn = db::test_connection();

        let err = delete_deck_inner(&conn, "does-not-exist").unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn delete_deck_cascades_to_its_cards_and_their_review_state() {
        let conn = db::test_connection();
        let deck = create_deck_inner(&conn, new_deck_input("Chinese HSK1")).unwrap();

        let now = Utc::now();
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
            created_at: now,
            updated_at: now,
        };
        db::cards::insert(&conn, &card).unwrap();
        let fsrs_state = crate::srs::new_card_state(now);
        db::review_state::seed(&conn, &card.id, Direction::FaceOneToTwo, &fsrs_state).unwrap();
        db::review_state::seed(&conn, &card.id, Direction::FaceTwoToOne, &fsrs_state).unwrap();

        delete_deck_inner(&conn, &deck.id).unwrap();

        assert!(db::cards::get(&conn, &card.id).unwrap().is_none());
        assert!(
            db::review_state::get(&conn, &card.id, Direction::FaceOneToTwo)
                .unwrap()
                .is_none()
        );
        assert!(
            db::review_state::get(&conn, &card.id, Direction::FaceTwoToOne)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn list_decks_orders_case_insensitively_by_name() {
        let conn = db::test_connection();
        create_deck_inner(&conn, new_deck_input("physics")).unwrap();
        create_deck_inner(&conn, new_deck_input("Chinese")).unwrap();
        create_deck_inner(&conn, new_deck_input("algebra")).unwrap();

        let names: Vec<String> = db::decks::list(&conn)
            .unwrap()
            .into_iter()
            .map(|d| d.name)
            .collect();

        assert_eq!(names, vec!["algebra", "Chinese", "physics"]);
    }
}
