use crate::error::AppResult;
use crate::models::card::Direction;
use chrono::{DateTime, Utc};
use rs_fsrs::{Card as FsrsCard, Rating, ReviewLog, State};
use rusqlite::{params, Connection, OptionalExtension, Row};

pub fn state_to_str(state: State) -> &'static str {
    match state {
        State::New => "New",
        State::Learning => "Learning",
        State::Review => "Review",
        State::Relearning => "Relearning",
    }
}

fn str_to_state(s: &str) -> State {
    match s {
        "Learning" => State::Learning,
        "Review" => State::Review,
        "Relearning" => State::Relearning,
        _ => State::New,
    }
}

fn rating_to_str(rating: Rating) -> &'static str {
    match rating {
        Rating::Again => "Again",
        Rating::Hard => "Hard",
        Rating::Good => "Good",
        Rating::Easy => "Easy",
    }
}

fn map_fsrs_card_row(row: &Row) -> rusqlite::Result<FsrsCard> {
    Ok(FsrsCard {
        due: row.get("due")?,
        stability: row.get("stability")?,
        difficulty: row.get("difficulty")?,
        elapsed_days: row.get("elapsed_days")?,
        scheduled_days: row.get("scheduled_days")?,
        reps: row.get("reps")?,
        lapses: row.get("lapses")?,
        state: str_to_state(&row.get::<_, String>("state")?),
        last_review: row.get("last_review")?,
    })
}

/// Inserts the initial FSRS state for a newly created card+direction pair.
pub fn seed(
    conn: &Connection,
    card_id: &str,
    direction: Direction,
    card: &FsrsCard,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO review_state (card_id, direction, due, stability, difficulty, elapsed_days, scheduled_days, reps, lapses, state, last_review)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            card_id,
            direction.as_str(),
            card.due,
            card.stability,
            card.difficulty,
            card.elapsed_days,
            card.scheduled_days,
            card.reps,
            card.lapses,
            state_to_str(card.state),
            card.last_review,
        ],
    )?;
    Ok(())
}

/// Overwrites the FSRS state for a card+direction pair after a review.
pub fn upsert(
    conn: &Connection,
    card_id: &str,
    direction: Direction,
    card: &FsrsCard,
) -> AppResult<()> {
    conn.execute(
        "UPDATE review_state SET due = ?1, stability = ?2, difficulty = ?3, elapsed_days = ?4,
         scheduled_days = ?5, reps = ?6, lapses = ?7, state = ?8, last_review = ?9
         WHERE card_id = ?10 AND direction = ?11",
        params![
            card.due,
            card.stability,
            card.difficulty,
            card.elapsed_days,
            card.scheduled_days,
            card.reps,
            card.lapses,
            state_to_str(card.state),
            card.last_review,
            card_id,
            direction.as_str(),
        ],
    )?;
    Ok(())
}

pub fn existing_directions(conn: &Connection, card_id: &str) -> AppResult<Vec<Direction>> {
    let mut stmt = conn.prepare("SELECT direction FROM review_state WHERE card_id = ?1")?;
    let rows = stmt
        .query_map(params![card_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|d| Direction::parse(&d))
        .collect())
}

/// Seeds fresh FSRS state (as of `now`) for any of `directions` that don't
/// already have review_state. A no-op for directions that are already seeded.
pub fn seed_missing(
    conn: &Connection,
    card_id: &str,
    directions: &[Direction],
    now: DateTime<Utc>,
) -> AppResult<()> {
    let existing = existing_directions(conn, card_id)?;
    let fsrs_state = crate::srs::new_card_state(now);
    for direction in directions {
        if !existing.contains(direction) {
            seed(conn, card_id, *direction, &fsrs_state)?;
        }
    }
    Ok(())
}

/// Deletes review_state for any currently-seeded direction that isn't in `directions`.
pub fn prune_removed(conn: &Connection, card_id: &str, directions: &[Direction]) -> AppResult<()> {
    for direction in existing_directions(conn, card_id)? {
        if !directions.contains(&direction) {
            delete(conn, card_id, direction)?;
        }
    }
    Ok(())
}

pub fn delete(conn: &Connection, card_id: &str, direction: Direction) -> AppResult<()> {
    conn.execute(
        "DELETE FROM review_state WHERE card_id = ?1 AND direction = ?2",
        params![card_id, direction.as_str()],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, card_id: &str, direction: Direction) -> AppResult<Option<FsrsCard>> {
    conn.query_row(
        "SELECT due, stability, difficulty, elapsed_days, scheduled_days, reps, lapses, state, last_review
         FROM review_state WHERE card_id = ?1 AND direction = ?2",
        params![card_id, direction.as_str()],
        map_fsrs_card_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn log_review(
    conn: &Connection,
    card_id: &str,
    direction: Direction,
    log: &ReviewLog,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO review_log (card_id, direction, rating, elapsed_days, scheduled_days, state, reviewed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            card_id,
            direction.as_str(),
            rating_to_str(log.rating),
            log.elapsed_days,
            log.scheduled_days,
            state_to_str(log.state),
            log.reviewed_date,
        ],
    )?;
    Ok(())
}

/// Every review item (state != New) currently due (`due <= now`) across all
/// decks, or a single deck if `deck_id` is given. Unbounded by design — a
/// study session needs to know the true count of everything due today, not
/// a SQL-capped slice of it; callers page through the (already-shuffled)
/// result themselves. Ordered oldest-due first as a deterministic base,
/// before any shuffling. Excludes items still awaiting their first review —
/// those are introduced separately, see `commands::study::select_new_items`.
pub fn due_review_items(
    conn: &Connection,
    deck_id: Option<&str>,
    now: DateTime<Utc>,
) -> AppResult<Vec<(String, Direction)>> {
    let mut stmt = conn.prepare(
        "SELECT rs.card_id, rs.direction FROM review_state rs
         JOIN cards c ON c.id = rs.card_id
         WHERE rs.due <= ?1 AND rs.state != 'New' AND (?2 IS NULL OR c.deck_id = ?2)
         ORDER BY rs.due ASC, rs.card_id ASC",
    )?;
    let rows = stmt
        .query_map(params![now, deck_id], |row| {
            let direction: String = row.get(1)?;
            Ok((row.get::<_, String>(0)?, direction))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows
        .into_iter()
        .filter_map(|(card_id, direction)| Direction::parse(&direction).map(|d| (card_id, d)))
        .collect())
}

/// Review items due later than `now` but no later than `until` — the "study
/// ahead of schedule" bonus pool offered once a session's normal due items
/// are exhausted. Disjoint from `due_review_items` by construction (`due >
/// now` here vs `due <= now` there), so the two never overlap.
pub fn ahead_review_items(
    conn: &Connection,
    deck_id: Option<&str>,
    now: DateTime<Utc>,
    until: DateTime<Utc>,
) -> AppResult<Vec<(String, Direction)>> {
    let mut stmt = conn.prepare(
        "SELECT rs.card_id, rs.direction FROM review_state rs
         JOIN cards c ON c.id = rs.card_id
         WHERE rs.due > ?1 AND rs.due <= ?2 AND rs.state != 'New' AND (?3 IS NULL OR c.deck_id = ?3)
         ORDER BY rs.due ASC, rs.card_id ASC",
    )?;
    let rows = stmt
        .query_map(params![now, until, deck_id], |row| {
            let direction: String = row.get(1)?;
            Ok((row.get::<_, String>(0)?, direction))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows
        .into_iter()
        .filter_map(|(card_id, direction)| Direction::parse(&direction).map(|d| (card_id, d)))
        .collect())
}

/// New (never-yet-reviewed) items across all decks, or a single deck if
/// `deck_id` is given, oldest-due first. Used for the `deck_id: None`
/// "everything due" mode, which doesn't yet have a UI and so isn't subject
/// to per-deck leveling/daily caps.
pub fn due_new_items(
    conn: &Connection,
    deck_id: Option<&str>,
    now: DateTime<Utc>,
    limit: i64,
) -> AppResult<Vec<(String, Direction)>> {
    let mut stmt = conn.prepare(
        "SELECT rs.card_id, rs.direction FROM review_state rs
         JOIN cards c ON c.id = rs.card_id
         WHERE rs.due <= ?1 AND rs.state = 'New' AND (?2 IS NULL OR c.deck_id = ?2)
         ORDER BY rs.due ASC
         LIMIT ?3",
    )?;
    let rows = stmt
        .query_map(params![now, deck_id, limit], |row| {
            let direction: String = row.get(1)?;
            Ok((row.get::<_, String>(0)?, direction))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows
        .into_iter()
        .filter_map(|(card_id, direction)| Direction::parse(&direction).map(|d| (card_id, d)))
        .collect())
}

/// Count of distinct cards in `deck_id` whose first-ever review happened on
/// the same calendar day as `now`, used to enforce a deck's daily new-card cap.
pub fn new_cards_introduced_today(
    conn: &Connection,
    deck_id: &str,
    now: DateTime<Utc>,
) -> AppResult<i64> {
    conn.query_row(
        "SELECT COUNT(DISTINCT rs.card_id) FROM review_state rs
         JOIN cards c ON c.id = rs.card_id
         WHERE c.deck_id = ?1 AND rs.reps = 1 AND date(rs.last_review) = date(?2)",
        params![deck_id, now],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::decks::tests_support::new_deck;
    use crate::db;
    use crate::models::card::{Card, CardFull};
    use chrono::Duration;
    use uuid::Uuid;

    fn new_card(conn: &Connection, deck_id: &str) -> Card {
        let card = Card {
            id: Uuid::new_v4().to_string(),
            deck_id: deck_id.to_string(),
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
        crate::db::cards::insert(conn, &card).unwrap();
        card
    }

    /// Seeds a card+direction directly into the Review state with one rep,
    /// as if it had already been reviewed once — `due_review_items` only
    /// considers items past this point.
    fn seed_reviewed(conn: &Connection, card_id: &str, direction: Direction, due: DateTime<Utc>) {
        let mut fsrs_card = crate::srs::new_card_state(due);
        fsrs_card.state = State::Review;
        fsrs_card.reps = 1;
        fsrs_card.due = due;
        seed(conn, card_id, direction, &fsrs_card).unwrap();
    }

    #[test]
    fn state_str_round_trips_for_every_variant() {
        for state in [
            State::New,
            State::Learning,
            State::Review,
            State::Relearning,
        ] {
            assert_eq!(str_to_state(state_to_str(state)), state);
        }
    }

    #[test]
    fn state_str_falls_back_to_new_for_an_unrecognized_value() {
        assert_eq!(str_to_state("garbage"), State::New);
    }

    #[test]
    fn rating_str_is_distinct_for_every_variant() {
        let strs: Vec<&str> = [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy]
            .into_iter()
            .map(rating_to_str)
            .collect();
        assert_eq!(strs, vec!["Again", "Hard", "Good", "Easy"]);
    }

    #[test]
    fn seed_then_get_round_trips_all_fields() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id);

        let mut fsrs_card = crate::srs::new_card_state(Utc::now());
        fsrs_card.stability = 3.5;
        fsrs_card.difficulty = 4.2;
        fsrs_card.reps = 2;
        fsrs_card.lapses = 1;
        fsrs_card.state = State::Review;

        seed(&conn, &card.id, Direction::FaceOneToTwo, &fsrs_card).unwrap();
        let reloaded = get(&conn, &card.id, Direction::FaceOneToTwo)
            .unwrap()
            .unwrap();

        assert_eq!(reloaded.stability, 3.5);
        assert_eq!(reloaded.difficulty, 4.2);
        assert_eq!(reloaded.reps, 2);
        assert_eq!(reloaded.lapses, 1);
        assert_eq!(reloaded.state, State::Review);
    }

    #[test]
    fn upsert_overwrites_the_existing_row() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id);
        let fsrs_card = crate::srs::new_card_state(Utc::now());
        seed(&conn, &card.id, Direction::FaceOneToTwo, &fsrs_card).unwrap();

        let mut updated = fsrs_card.clone();
        updated.reps = 5;
        upsert(&conn, &card.id, Direction::FaceOneToTwo, &updated).unwrap();

        let reloaded = get(&conn, &card.id, Direction::FaceOneToTwo)
            .unwrap()
            .unwrap();
        assert_eq!(reloaded.reps, 5);
    }

    #[test]
    fn get_returns_none_for_an_unseeded_direction() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id);

        assert!(get(&conn, &card.id, Direction::FaceOneToTwo)
            .unwrap()
            .is_none());
    }

    #[test]
    fn existing_directions_returns_only_the_seeded_ones() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id);
        let fsrs_card = crate::srs::new_card_state(Utc::now());
        seed(&conn, &card.id, Direction::FaceOneToTwo, &fsrs_card).unwrap();

        assert_eq!(
            existing_directions(&conn, &card.id).unwrap(),
            vec![Direction::FaceOneToTwo]
        );
    }

    #[test]
    fn delete_removes_only_the_specified_direction() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id);
        let fsrs_card = crate::srs::new_card_state(Utc::now());
        seed(&conn, &card.id, Direction::FaceOneToTwo, &fsrs_card).unwrap();
        seed(&conn, &card.id, Direction::FaceTwoToOne, &fsrs_card).unwrap();

        delete(&conn, &card.id, Direction::FaceOneToTwo).unwrap();

        assert!(get(&conn, &card.id, Direction::FaceOneToTwo)
            .unwrap()
            .is_none());
        assert!(get(&conn, &card.id, Direction::FaceTwoToOne)
            .unwrap()
            .is_some());
    }

    #[test]
    fn log_review_writes_a_row_with_the_expected_fields() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id);
        let log = ReviewLog {
            rating: Rating::Good,
            elapsed_days: 3,
            scheduled_days: 7,
            state: State::Review,
            reviewed_date: Utc::now(),
        };

        log_review(&conn, &card.id, Direction::FaceOneToTwo, &log).unwrap();

        let (rating, elapsed, scheduled): (String, i64, i64) = conn
            .query_row(
                "SELECT rating, elapsed_days, scheduled_days FROM review_log WHERE card_id = ?1",
                params![card.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(rating, "Good");
        assert_eq!(elapsed, 3);
        assert_eq!(scheduled, 7);
    }

    #[test]
    fn due_review_items_only_returns_items_due_now_or_earlier() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id);
        let now = Utc::now();

        seed_reviewed(
            &conn,
            &card.id,
            Direction::FaceOneToTwo,
            now - Duration::days(1),
        );
        seed_reviewed(
            &conn,
            &card.id,
            Direction::FaceTwoToOne,
            now + Duration::days(1),
        );

        let due = due_review_items(&conn, None, now).unwrap();

        assert_eq!(due, vec![(card.id.clone(), Direction::FaceOneToTwo)]);
    }

    #[test]
    fn due_review_items_excludes_cards_still_in_the_new_state() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id);
        let now = Utc::now();
        let due_state = crate::srs::new_card_state(now);
        seed(&conn, &card.id, Direction::FaceOneToTwo, &due_state).unwrap();

        let due = due_review_items(&conn, None, now).unwrap();

        assert!(due.is_empty());
    }

    #[test]
    fn due_review_items_filters_by_deck_id() {
        let conn = db::test_connection();
        let deck_a = new_deck(&conn, "Deck A");
        let deck_b = new_deck(&conn, "Deck B");
        let card_a = new_card(&conn, &deck_a.id);
        let card_b = new_card(&conn, &deck_b.id);
        let now = Utc::now();
        seed_reviewed(&conn, &card_a.id, Direction::FaceOneToTwo, now);
        seed_reviewed(&conn, &card_b.id, Direction::FaceOneToTwo, now);

        let due = due_review_items(&conn, Some(&deck_a.id), now).unwrap();

        assert_eq!(due, vec![(card_a.id.clone(), Direction::FaceOneToTwo)]);
    }

    #[test]
    fn due_review_items_returns_every_due_item_unbounded() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let now = Utc::now();
        for _ in 0..30 {
            let card = new_card(&conn, &deck.id);
            seed_reviewed(&conn, &card.id, Direction::FaceOneToTwo, now);
        }

        let due = due_review_items(&conn, None, now).unwrap();

        assert_eq!(due.len(), 30);
    }

    #[test]
    fn due_review_items_orders_oldest_due_first() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card_a = new_card(&conn, &deck.id);
        let card_b = new_card(&conn, &deck.id);
        let now = Utc::now();

        seed_reviewed(
            &conn,
            &card_a.id,
            Direction::FaceOneToTwo,
            now - Duration::days(5),
        );
        seed_reviewed(
            &conn,
            &card_b.id,
            Direction::FaceOneToTwo,
            now - Duration::days(1),
        );

        let due = due_review_items(&conn, None, now).unwrap();

        assert_eq!(
            due,
            vec![
                (card_a.id.clone(), Direction::FaceOneToTwo),
                (card_b.id.clone(), Direction::FaceOneToTwo),
            ]
        );
    }

    #[test]
    fn ahead_review_items_only_returns_items_due_after_now_and_within_the_window() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card_overdue = new_card(&conn, &deck.id);
        let card_within_window = new_card(&conn, &deck.id);
        let card_beyond_window = new_card(&conn, &deck.id);
        let now = Utc::now();
        let until = now + Duration::hours(24);

        seed_reviewed(
            &conn,
            &card_overdue.id,
            Direction::FaceOneToTwo,
            now - Duration::hours(1),
        );
        seed_reviewed(
            &conn,
            &card_within_window.id,
            Direction::FaceOneToTwo,
            now + Duration::hours(12),
        );
        seed_reviewed(
            &conn,
            &card_beyond_window.id,
            Direction::FaceOneToTwo,
            now + Duration::hours(48),
        );

        let ahead = ahead_review_items(&conn, None, now, until).unwrap();

        assert_eq!(
            ahead,
            vec![(card_within_window.id.clone(), Direction::FaceOneToTwo)]
        );
    }

    #[test]
    fn ahead_review_items_filters_by_deck_id() {
        let conn = db::test_connection();
        let deck_a = new_deck(&conn, "Deck A");
        let deck_b = new_deck(&conn, "Deck B");
        let card_a = new_card(&conn, &deck_a.id);
        let card_b = new_card(&conn, &deck_b.id);
        let now = Utc::now();
        let until = now + Duration::hours(24);
        seed_reviewed(
            &conn,
            &card_a.id,
            Direction::FaceOneToTwo,
            now + Duration::hours(1),
        );
        seed_reviewed(
            &conn,
            &card_b.id,
            Direction::FaceOneToTwo,
            now + Duration::hours(1),
        );

        let ahead = ahead_review_items(&conn, Some(&deck_a.id), now, until).unwrap();

        assert_eq!(ahead, vec![(card_a.id.clone(), Direction::FaceOneToTwo)]);
    }

    #[test]
    fn due_new_items_only_returns_cards_still_in_the_new_state() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let new_card_ = new_card(&conn, &deck.id);
        let reviewed_card = new_card(&conn, &deck.id);
        let now = Utc::now();
        let due_state = crate::srs::new_card_state(now);
        seed(&conn, &new_card_.id, Direction::FaceOneToTwo, &due_state).unwrap();
        seed_reviewed(&conn, &reviewed_card.id, Direction::FaceOneToTwo, now);

        let due = due_new_items(&conn, None, now, 10).unwrap();

        assert_eq!(due, vec![(new_card_.id.clone(), Direction::FaceOneToTwo)]);
    }

    #[test]
    fn new_cards_introduced_today_counts_distinct_cards_not_items() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id);
        let now = Utc::now();
        // Both directions of the same card reviewed for the first time today.
        seed_reviewed(&conn, &card.id, Direction::FaceOneToTwo, now);
        seed_reviewed(&conn, &card.id, Direction::FaceTwoToOne, now);

        let count = new_cards_introduced_today(&conn, &deck.id, now).unwrap();

        assert_eq!(count, 1);
    }

    #[test]
    fn new_cards_introduced_today_ignores_cards_reviewed_on_a_previous_day() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id);
        let now = Utc::now();
        seed_reviewed(
            &conn,
            &card.id,
            Direction::FaceOneToTwo,
            now - Duration::days(1),
        );

        let count = new_cards_introduced_today(&conn, &deck.id, now).unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn new_cards_introduced_today_ignores_cards_never_reviewed() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id);
        let now = Utc::now();
        let due_state = crate::srs::new_card_state(now);
        seed(&conn, &card.id, Direction::FaceOneToTwo, &due_state).unwrap();

        let count = new_cards_introduced_today(&conn, &deck.id, now).unwrap();

        assert_eq!(count, 0);
    }
}
