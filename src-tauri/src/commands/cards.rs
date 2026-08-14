use super::AppState;
use crate::db;
use crate::db::cards::{CardFilter, CardPage};
use crate::error::{AppError, AppResult};
use crate::models::card::{Card, NewCard, UpdateCard};
use chrono::Utc;
use rusqlite::Connection;
use tauri::State;
use uuid::Uuid;

fn validate(
    face_1: &str,
    face_2: &str,
    directions: &[crate::models::card::Direction],
) -> AppResult<()> {
    if face_1.trim().is_empty() || face_2.trim().is_empty() {
        return Err(AppError::InvalidInput("both faces are required".into()));
    }
    if directions.is_empty() {
        return Err(AppError::InvalidInput(
            "a card needs at least one guessable direction".into(),
        ));
    }
    Ok(())
}

fn create_card_inner(conn: &Connection, input: NewCard) -> AppResult<Card> {
    validate(&input.face_1, &input.face_2, &input.directions)?;

    let now = Utc::now();
    let card = Card {
        id: Uuid::new_v4().to_string(),
        deck_id: input.deck_id,
        face_1: input.face_1,
        face_2: input.face_2,
        full: input.full,
        tags: input.tags,
        directions: input.directions,
        level: input.level,
        created_at: now,
        updated_at: now,
    };
    db::cards::insert(conn, &card)?;
    db::review_state::seed_missing(conn, &card.id, &card.directions, now)?;

    Ok(card)
}

fn update_card_inner(conn: &Connection, input: UpdateCard) -> AppResult<Card> {
    validate(&input.face_1, &input.face_2, &input.directions)?;

    let existing = db::cards::get(conn, &input.id)?
        .ok_or_else(|| AppError::NotFound(format!("card {}", input.id)))?;
    let now = Utc::now();
    let updated = Card {
        face_1: input.face_1,
        face_2: input.face_2,
        full: input.full,
        tags: input.tags,
        directions: input.directions,
        level: input.level,
        updated_at: now,
        ..existing
    };
    db::cards::update(conn, &updated)?;

    db::review_state::seed_missing(conn, &updated.id, &updated.directions, now)?;
    db::review_state::prune_removed(conn, &updated.id, &updated.directions)?;

    Ok(updated)
}

fn delete_card_inner(conn: &Connection, id: &str) -> AppResult<()> {
    if !db::cards::delete(conn, id)? {
        return Err(AppError::NotFound(format!("card {id}")));
    }
    Ok(())
}

/// `limit`/`offset` default to "everything" when omitted, so existing
/// callers that only pass `deck_id` keep working while the browse UI grows
/// pagination on top of this.
#[tauri::command]
pub fn list_cards(
    state: State<AppState>,
    deck_id: String,
    search: Option<String>,
    tags: Option<Vec<String>>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<CardPage> {
    let conn = state.conn();
    let filter = CardFilter {
        search,
        tags: tags.unwrap_or_default(),
    };
    db::cards::browse(
        &conn,
        &deck_id,
        &filter,
        limit.unwrap_or(i64::MAX),
        offset.unwrap_or(0),
    )
}

#[tauri::command]
pub fn list_deck_tags(state: State<AppState>, deck_id: String) -> AppResult<Vec<String>> {
    let conn = state.conn();
    db::cards::distinct_tags(&conn, &deck_id)
}

#[tauri::command]
pub fn create_card(state: State<AppState>, input: NewCard) -> AppResult<Card> {
    let conn = state.conn();
    create_card_inner(&conn, input)
}

#[tauri::command]
pub fn update_card(state: State<AppState>, input: UpdateCard) -> AppResult<Card> {
    let conn = state.conn();
    update_card_inner(&conn, input)
}

#[tauri::command]
pub fn delete_card(state: State<AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    delete_card_inner(&conn, &id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::card::{CardFull, Direction};
    use crate::models::deck::Deck;

    fn setup() -> (Connection, String) {
        let conn = db::test_connection();
        let deck = Deck {
            id: Uuid::new_v4().to_string(),
            name: "Test deck".into(),
            description: None,
            new_cards_per_day: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db::decks::insert(&conn, &deck).unwrap();
        (conn, deck.id)
    }

    fn blank_full() -> CardFull {
        CardFull {
            title: "狗".into(),
            subtitle: "gǒu".into(),
            body: "Domestic dog.".into(),
            foot: String::new(),
        }
    }

    fn new_card_input(deck_id: &str, directions: Vec<Direction>) -> NewCard {
        NewCard {
            deck_id: deck_id.to_string(),
            face_1: "dog".into(),
            face_2: "狗".into(),
            full: blank_full(),
            tags: vec!["animals".into()],
            directions,
            level: 1,
        }
    }

    #[test]
    fn create_card_seeds_review_state_for_each_requested_direction() {
        let (conn, deck_id) = setup();

        let card =
            create_card_inner(&conn, new_card_input(&deck_id, Direction::all().to_vec())).unwrap();

        assert!(
            db::review_state::get(&conn, &card.id, Direction::FaceOneToTwo)
                .unwrap()
                .is_some()
        );
        assert!(
            db::review_state::get(&conn, &card.id, Direction::FaceTwoToOne)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn create_card_seeds_only_the_requested_direction() {
        let (conn, deck_id) = setup();

        let card = create_card_inner(
            &conn,
            new_card_input(&deck_id, vec![Direction::FaceOneToTwo]),
        )
        .unwrap();

        assert!(
            db::review_state::get(&conn, &card.id, Direction::FaceOneToTwo)
                .unwrap()
                .is_some()
        );
        assert!(
            db::review_state::get(&conn, &card.id, Direction::FaceTwoToOne)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn create_card_rejects_empty_faces() {
        let (conn, deck_id) = setup();

        let mut input = new_card_input(&deck_id, Direction::all().to_vec());
        input.face_2 = "  ".into();

        let err = create_card_inner(&conn, input).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn create_card_rejects_no_directions() {
        let (conn, deck_id) = setup();

        let err = create_card_inner(&conn, new_card_input(&deck_id, vec![])).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn update_card_persists_field_changes() {
        let (conn, deck_id) = setup();
        let card =
            create_card_inner(&conn, new_card_input(&deck_id, Direction::all().to_vec())).unwrap();

        let updated = update_card_inner(
            &conn,
            UpdateCard {
                id: card.id.clone(),
                face_1: "dog (edited)".into(),
                face_2: card.face_2.clone(),
                full: card.full.clone(),
                tags: vec!["animals".into(), "hsk1".into()],
                directions: Direction::all().to_vec(),
                level: card.level,
            },
        )
        .unwrap();

        assert_eq!(updated.face_1, "dog (edited)");
        assert_eq!(updated.tags, vec!["animals", "hsk1"]);
        let reloaded = db::cards::get(&conn, &card.id).unwrap().unwrap();
        assert_eq!(reloaded.face_1, "dog (edited)");
    }

    #[test]
    fn update_card_seeds_review_state_for_a_newly_added_direction() {
        let (conn, deck_id) = setup();
        let card = create_card_inner(
            &conn,
            new_card_input(&deck_id, vec![Direction::FaceOneToTwo]),
        )
        .unwrap();

        update_card_inner(
            &conn,
            UpdateCard {
                id: card.id.clone(),
                face_1: card.face_1.clone(),
                face_2: card.face_2.clone(),
                full: card.full.clone(),
                tags: card.tags.clone(),
                directions: Direction::all().to_vec(),
                level: card.level,
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
    fn update_card_does_not_reset_progress_on_an_untouched_direction() {
        let (conn, deck_id) = setup();
        let card =
            create_card_inner(&conn, new_card_input(&deck_id, Direction::all().to_vec())).unwrap();

        // Simulate some review progress on one direction before the edit.
        let mut reviewed = db::review_state::get(&conn, &card.id, Direction::FaceOneToTwo)
            .unwrap()
            .unwrap();
        reviewed.reps = 7;
        db::review_state::upsert(&conn, &card.id, Direction::FaceOneToTwo, &reviewed).unwrap();

        update_card_inner(
            &conn,
            UpdateCard {
                id: card.id.clone(),
                face_1: "dog (edited)".into(),
                face_2: card.face_2.clone(),
                full: card.full.clone(),
                tags: card.tags.clone(),
                directions: Direction::all().to_vec(),
                level: card.level,
            },
        )
        .unwrap();

        let after = db::review_state::get(&conn, &card.id, Direction::FaceOneToTwo)
            .unwrap()
            .unwrap();
        assert_eq!(after.reps, 7);
    }

    #[test]
    fn update_card_drops_review_state_for_a_removed_direction() {
        let (conn, deck_id) = setup();
        let card =
            create_card_inner(&conn, new_card_input(&deck_id, Direction::all().to_vec())).unwrap();

        update_card_inner(
            &conn,
            UpdateCard {
                id: card.id.clone(),
                face_1: card.face_1.clone(),
                face_2: card.face_2.clone(),
                full: card.full.clone(),
                tags: card.tags.clone(),
                directions: vec![Direction::FaceOneToTwo],
                level: card.level,
            },
        )
        .unwrap();

        assert!(
            db::review_state::get(&conn, &card.id, Direction::FaceTwoToOne)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn update_card_rejects_empty_faces() {
        let (conn, deck_id) = setup();
        let card =
            create_card_inner(&conn, new_card_input(&deck_id, Direction::all().to_vec())).unwrap();

        let err = update_card_inner(
            &conn,
            UpdateCard {
                id: card.id,
                face_1: String::new(),
                face_2: card.face_2,
                full: card.full,
                tags: card.tags,
                directions: card.directions,
                level: card.level,
            },
        )
        .unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn update_card_returns_not_found_for_a_missing_id() {
        let (conn, _deck_id) = setup();

        let err = update_card_inner(
            &conn,
            UpdateCard {
                id: "does-not-exist".into(),
                face_1: "dog".into(),
                face_2: "狗".into(),
                full: blank_full(),
                tags: vec![],
                directions: Direction::all().to_vec(),
                level: 1,
            },
        )
        .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn delete_card_removes_the_card_and_cascades_its_review_state() {
        let (conn, deck_id) = setup();
        let card =
            create_card_inner(&conn, new_card_input(&deck_id, Direction::all().to_vec())).unwrap();

        delete_card_inner(&conn, &card.id).unwrap();

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
    fn delete_card_returns_not_found_for_a_missing_id() {
        let (conn, _deck_id) = setup();

        let err = delete_card_inner(&conn, "does-not-exist").unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }
}
