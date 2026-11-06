use crate::error::AppResult;
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RatingCounts {
    pub again: i64,
    pub hard: i64,
    pub good: i64,
    pub easy: i64,
}

/// `(total cards, cards with at least one direction that's been reviewed at
/// least once)` for `deck_id`.
pub fn card_introduction_counts(conn: &Connection, deck_id: &str) -> AppResult<(i64, i64)> {
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cards WHERE deck_id = ?1",
        params![deck_id],
        |row| row.get(0),
    )?;
    let introduced: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT rs.card_id) FROM review_state rs
         JOIN cards c ON c.id = rs.card_id
         WHERE c.deck_id = ?1 AND rs.state != 'New'",
        params![deck_id],
        |row| row.get(0),
    )?;
    Ok((total, introduced))
}

/// Buckets every *reviewed* item (one per card+direction, excluding
/// anything still `New` — never shown a rating, so there's nothing to
/// bucket it by) in `deck_id` by its most recent rating. An item with no
/// matching log entry despite having left `New` — in principle possible,
/// never actually reachable — defaults to `Again` rather than being
/// silently dropped.
pub fn rating_counts(conn: &Connection, deck_id: &str) -> AppResult<RatingCounts> {
    let mut stmt = conn.prepare(
        "SELECT latest.rating
         FROM review_state rs
         JOIN cards c ON c.id = rs.card_id
         LEFT JOIN (
             SELECT rl.card_id, rl.direction, rl.rating
             FROM review_log rl
             JOIN (
                 SELECT card_id, direction, MAX(id) AS max_id
                 FROM review_log
                 GROUP BY card_id, direction
             ) latest_id ON latest_id.max_id = rl.id
         ) latest ON latest.card_id = rs.card_id AND latest.direction = rs.direction
         WHERE c.deck_id = ?1 AND rs.state != 'New'",
    )?;

    let mut counts = RatingCounts::default();
    let ratings = stmt.query_map(params![deck_id], |row| row.get::<_, Option<String>>(0))?;
    for rating in ratings {
        match rating?.as_deref() {
            Some("Hard") => counts.hard += 1,
            Some("Good") => counts.good += 1,
            Some("Easy") => counts.easy += 1,
            _ => counts.again += 1,
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::decks::tests_support::new_deck;
    use crate::db;
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

    fn rate(conn: &Connection, card_id: &str, direction: Direction, rating: Rating) {
        let current = db::review_state::get(conn, card_id, direction)
            .unwrap()
            .unwrap();
        let scheduling = crate::srs::review(current, Utc::now(), rating);
        db::review_state::upsert(conn, card_id, direction, &scheduling.card).unwrap();
        db::review_state::log_review(conn, card_id, direction, &scheduling.review_log).unwrap();
    }

    #[test]
    fn card_introduction_counts_counts_a_card_as_introduced_once_either_direction_is_reviewed() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let untouched = new_card(&conn, &deck.id, Direction::all().to_vec());
        let partially_reviewed = new_card(&conn, &deck.id, Direction::all().to_vec());
        rate(
            &conn,
            &partially_reviewed.id,
            Direction::FaceOneToTwo,
            Rating::Good,
        );
        let _ = untouched;

        let (total, introduced) = card_introduction_counts(&conn, &deck.id).unwrap();

        assert_eq!(total, 2);
        assert_eq!(introduced, 1);
    }

    #[test]
    fn card_introduction_counts_ignores_other_decks() {
        let conn = db::test_connection();
        let deck_a = new_deck(&conn, "Deck A");
        let deck_b = new_deck(&conn, "Deck B");
        new_card(&conn, &deck_a.id, Direction::all().to_vec());
        new_card(&conn, &deck_b.id, Direction::all().to_vec());
        new_card(&conn, &deck_b.id, Direction::all().to_vec());

        let (total, _) = card_introduction_counts(&conn, &deck_a.id).unwrap();

        assert_eq!(total, 1);
    }

    #[test]
    fn rating_counts_excludes_items_that_have_never_been_reviewed() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo]);

        let counts = rating_counts(&conn, &deck.id).unwrap();

        assert_eq!(counts, RatingCounts::default());
    }

    #[test]
    fn rating_counts_buckets_by_each_items_most_recent_rating() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, Direction::all().to_vec());
        rate(&conn, &card.id, Direction::FaceOneToTwo, Rating::Hard);
        rate(&conn, &card.id, Direction::FaceTwoToOne, Rating::Easy);

        let counts = rating_counts(&conn, &deck.id).unwrap();

        assert_eq!(
            counts,
            RatingCounts {
                hard: 1,
                easy: 1,
                ..Default::default()
            }
        );
    }

    #[test]
    fn rating_counts_uses_only_the_latest_rating_per_item() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo]);
        rate(&conn, &card.id, Direction::FaceOneToTwo, Rating::Again);
        rate(&conn, &card.id, Direction::FaceOneToTwo, Rating::Good);

        let counts = rating_counts(&conn, &deck.id).unwrap();

        assert_eq!(
            counts,
            RatingCounts {
                good: 1,
                ..Default::default()
            }
        );
    }

    #[test]
    fn rating_counts_counts_each_direction_as_its_own_item_and_excludes_the_unreviewed_one() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, Direction::all().to_vec());
        rate(&conn, &card.id, Direction::FaceOneToTwo, Rating::Easy);
        // FaceTwoToOne is left unreviewed and should not show up at all.

        let counts = rating_counts(&conn, &deck.id).unwrap();

        assert_eq!(
            counts,
            RatingCounts {
                easy: 1,
                ..Default::default()
            }
        );
    }

    #[test]
    fn rating_counts_ignores_other_decks() {
        let conn = db::test_connection();
        let deck_a = new_deck(&conn, "Deck A");
        let deck_b = new_deck(&conn, "Deck B");
        let card_a = new_card(&conn, &deck_a.id, vec![Direction::FaceOneToTwo]);
        rate(&conn, &card_a.id, Direction::FaceOneToTwo, Rating::Good);
        let card_b = new_card(&conn, &deck_b.id, vec![Direction::FaceOneToTwo]);
        rate(&conn, &card_b.id, Direction::FaceOneToTwo, Rating::Easy);

        let counts = rating_counts(&conn, &deck_a.id).unwrap();

        assert_eq!(
            counts,
            RatingCounts {
                good: 1,
                ..Default::default()
            }
        );
    }
}
