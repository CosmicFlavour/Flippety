use super::AppState;
use crate::db;
use crate::db::stats::RatingCounts;
use crate::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct DeckStats {
    pub total_cards: i64,
    pub introduced_cards: i64,
    pub ratings: RatingCounts,
}

fn get_deck_stats_inner(conn: &Connection, deck_id: &str) -> AppResult<DeckStats> {
    let (total_cards, introduced_cards) = db::stats::card_introduction_counts(conn, deck_id)?;
    let ratings = db::stats::rating_counts(conn, deck_id)?;
    Ok(DeckStats {
        total_cards,
        introduced_cards,
        ratings,
    })
}

#[tauri::command]
pub fn get_deck_stats(state: State<AppState>, deck_id: String) -> AppResult<DeckStats> {
    let conn = state.conn();
    get_deck_stats_inner(&conn, &deck_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::decks::tests_support::new_deck;
    use crate::models::card::{Card, CardFull, Direction};
    use chrono::Utc;
    use rs_fsrs::Rating;
    use uuid::Uuid;

    fn new_card(conn: &Connection, deck_id: &str, directions: Vec<Direction>) -> Card {
        let now = Utc::now();
        let card = Card {
            id: Uuid::new_v4().to_string(),
            deck_id: deck_id.to_string(),
            face_1: "dog".into(),
            face_2: "狗".into(),
            full: CardFull {
                title: String::new(),
                subtitle: String::new(),
                body: String::new(),
                foot: String::new(),
            },
            tags: vec![],
            directions,
            level: 1,
            created_at: now,
            updated_at: now,
        };
        db::cards::insert(conn, &card).unwrap();
        let fsrs_state = crate::srs::new_card_state(now);
        for direction in &card.directions {
            db::review_state::seed(conn, &card.id, *direction, &fsrs_state).unwrap();
        }
        card
    }

    #[test]
    fn get_deck_stats_combines_card_counts_and_rating_counts() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, Direction::all().to_vec());
        new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo]);

        let current = db::review_state::get(&conn, &card.id, Direction::FaceOneToTwo)
            .unwrap()
            .unwrap();
        let scheduling = crate::srs::review(current, Utc::now(), Rating::Good);
        db::review_state::upsert(&conn, &card.id, Direction::FaceOneToTwo, &scheduling.card)
            .unwrap();
        db::review_state::log_review(
            &conn,
            &card.id,
            Direction::FaceOneToTwo,
            &scheduling.review_log,
        )
        .unwrap();

        let stats = get_deck_stats_inner(&conn, &deck.id).unwrap();

        assert_eq!(stats.total_cards, 2);
        assert_eq!(stats.introduced_cards, 1);
        assert_eq!(stats.ratings.good, 1);
        // The card's other direction and the untouched card are both still
        // New — excluded from the rating breakdown entirely.
        assert_eq!(stats.ratings.again, 0);
    }
}
