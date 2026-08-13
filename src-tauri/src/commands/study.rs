use super::AppState;
use crate::db;
use crate::db::cards::NewCardCandidate;
use crate::error::{AppError, AppResult};
use crate::models::card::{CardFull, Direction};
use chrono::{DateTime, Utc};
use rand::seq::SliceRandom;
use rs_fsrs::Rating;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

/// One due review: which card, which direction to prompt, and the content
/// needed to render both the prompt and (once revealed) the full solution.
#[derive(Debug, Clone, Serialize)]
pub struct DueItem {
    pub card_id: String,
    pub direction: Direction,
    pub prompt: String,
    pub full: CardFull,
}

/// Picks which new cards to introduce for `deck_id`, in level-ascending
/// order, clustered by a shared tag within a level (so introduction reads as
/// themed blocks rather than a shuffle) and capped at `per_day_cap` distinct
/// cards already introduced today (`None` = unlimited). A card contributes
/// all of its still-New directions at once, so the cap counts cards, not items.
fn select_new_items(
    conn: &Connection,
    deck_id: &str,
    now: DateTime<Utc>,
    per_day_cap: Option<i64>,
) -> AppResult<Vec<(String, Direction)>> {
    let candidates = db::cards::new_card_candidates(conn, deck_id)?;
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let budget = match per_day_cap {
        Some(cap) => {
            let introduced_today =
                db::review_state::new_cards_introduced_today(conn, deck_id, now)?;
            let remaining = (cap - introduced_today).max(0) as usize;
            if remaining == 0 {
                return Ok(Vec::new());
            }
            Some(remaining)
        }
        None => None,
    };

    // Candidates arrive level-ascending; group consecutive same-level runs.
    let mut level_groups: Vec<Vec<NewCardCandidate>> = Vec::new();
    for candidate in candidates {
        match level_groups.last_mut() {
            Some(group) if group[0].level == candidate.level => group.push(candidate),
            _ => level_groups.push(vec![candidate]),
        }
    }

    let mut rng = rand::thread_rng();
    let mut ordered: Vec<NewCardCandidate> = Vec::new();
    for group in level_groups {
        // Cluster by theme (the alphabetically-first tag; untagged cards
        // share a bucket), then shuffle both the theme order and each
        // theme's cards so introduction is randomized but still blocky.
        let mut by_theme: HashMap<Option<String>, Vec<NewCardCandidate>> = HashMap::new();
        for candidate in group {
            let theme = candidate.tags.iter().min().cloned();
            by_theme.entry(theme).or_default().push(candidate);
        }
        let mut theme_groups: Vec<Vec<NewCardCandidate>> = by_theme.into_values().collect();
        theme_groups.shuffle(&mut rng);
        for mut theme_group in theme_groups {
            theme_group.shuffle(&mut rng);
            ordered.extend(theme_group);
        }
    }

    let mut items = Vec::new();
    for (cards_taken, candidate) in ordered.into_iter().enumerate() {
        if budget.is_some_and(|budget| cards_taken >= budget) {
            break;
        }
        for direction in &candidate.directions {
            items.push((candidate.card_id.clone(), *direction));
        }
    }
    Ok(items)
}

fn get_due_queue_inner(
    conn: &Connection,
    deck_id: Option<&str>,
    limit: i64,
) -> AppResult<Vec<DueItem>> {
    let now = Utc::now();
    let mut due = db::review_state::due_review_items(conn, deck_id, now, limit)?;

    let remaining = limit - due.len() as i64;
    if remaining > 0 {
        let new_items = match deck_id {
            Some(deck_id) => {
                let per_day_cap = db::decks::get(conn, deck_id)?
                    .ok_or_else(|| AppError::NotFound(format!("deck {deck_id}")))?
                    .new_cards_per_day;
                let mut items = select_new_items(conn, deck_id, now, per_day_cap)?;
                items.truncate(remaining as usize);
                items
            }
            // "Everything due" mode has no UI yet, so new-card leveling and
            // the daily cap (both inherently per-deck) don't apply here.
            None => db::review_state::due_new_items(conn, None, now, remaining)?,
        };
        due.extend(new_items);
    }

    let mut items = Vec::with_capacity(due.len());
    for (card_id, direction) in due {
        let card = db::cards::get(conn, &card_id)?
            .ok_or_else(|| AppError::NotFound(format!("card {card_id}")))?;
        let prompt = match direction {
            Direction::FaceOneToTwo => card.face_1,
            Direction::FaceTwoToOne => card.face_2,
        };
        items.push(DueItem {
            card_id: card.id,
            direction,
            prompt,
            full: card.full,
        });
    }
    Ok(items)
}

#[tauri::command]
pub fn get_due_queue(
    state: State<AppState>,
    deck_id: Option<String>,
    limit: i64,
) -> AppResult<Vec<DueItem>> {
    let conn = state.conn();
    get_due_queue_inner(&conn, deck_id.as_deref(), limit)
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmitReviewInput {
    pub card_id: String,
    pub direction: Direction,
    pub rating: Rating,
}

fn submit_review_inner(conn: &Connection, input: SubmitReviewInput) -> AppResult<()> {
    let now = Utc::now();

    let current =
        db::review_state::get(conn, &input.card_id, input.direction)?.ok_or_else(|| {
            AppError::NotFound(format!(
                "review state for card {} direction {:?}",
                input.card_id, input.direction
            ))
        })?;

    let scheduling = crate::srs::review(current, now, input.rating);
    db::review_state::upsert(conn, &input.card_id, input.direction, &scheduling.card)?;
    db::review_state::log_review(
        conn,
        &input.card_id,
        input.direction,
        &scheduling.review_log,
    )?;
    Ok(())
}

#[tauri::command]
pub fn submit_review(state: State<AppState>, input: SubmitReviewInput) -> AppResult<()> {
    let conn = state.conn();
    submit_review_inner(&conn, input)
}

/// Clears a card's current FSRS scheduling and reseeds it as a fresh New
/// card in its current directions, so it re-enters new-card introduction
/// (e.g. after re-leveling a card that was already reviewed). Review
/// history in `review_log` is left untouched.
fn reset_card_progress_inner(conn: &Connection, card_id: &str) -> AppResult<()> {
    let card = db::cards::get(conn, card_id)?
        .ok_or_else(|| AppError::NotFound(format!("card {card_id}")))?;
    for direction in db::review_state::existing_directions(conn, &card.id)? {
        db::review_state::delete(conn, &card.id, direction)?;
    }
    db::review_state::seed_missing(conn, &card.id, &card.directions, Utc::now())?;
    Ok(())
}

#[tauri::command]
pub fn reset_card_progress(state: State<AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    reset_card_progress_inner(&conn, &id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::decks::tests_support::new_deck;
    use crate::models::card::{Card, CardFull};
    use rs_fsrs::State;
    use uuid::Uuid;

    fn new_card(conn: &Connection, deck_id: &str, directions: Vec<Direction>, level: i64) -> Card {
        let card = Card {
            id: Uuid::new_v4().to_string(),
            deck_id: deck_id.to_string(),
            face_1: "dog".into(),
            face_2: "狗".into(),
            full: CardFull {
                title: "狗".into(),
                subtitle: "gǒu".into(),
                body: "Domestic dog.".into(),
                foot: String::new(),
            },
            tags: vec![],
            directions,
            level,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db::cards::insert(conn, &card).unwrap();
        let fsrs_state = crate::srs::new_card_state(Utc::now());
        for direction in &card.directions {
            db::review_state::seed(conn, &card.id, *direction, &fsrs_state).unwrap();
        }
        card
    }

    #[test]
    fn due_queue_prompts_face_1_for_the_1_to_2_direction() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        let items = get_due_queue_inner(&conn, None, 10).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].card_id, card.id);
        assert_eq!(items[0].direction, Direction::FaceOneToTwo);
        assert_eq!(items[0].prompt, "dog");
        assert_eq!(items[0].full.title, "狗");
    }

    #[test]
    fn due_queue_prompts_face_2_for_the_2_to_1_direction() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        new_card(&conn, &deck.id, vec![Direction::FaceTwoToOne], 1);

        let items = get_due_queue_inner(&conn, None, 10).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].direction, Direction::FaceTwoToOne);
        assert_eq!(items[0].prompt, "狗");
    }

    #[test]
    fn due_queue_filters_by_deck() {
        let conn = db::test_connection();
        let deck_a = new_deck(&conn, "Deck A");
        let deck_b = new_deck(&conn, "Deck B");
        new_card(&conn, &deck_a.id, vec![Direction::FaceOneToTwo], 1);
        new_card(&conn, &deck_b.id, vec![Direction::FaceOneToTwo], 1);

        let items = get_due_queue_inner(&conn, Some(&deck_a.id), 10).unwrap();

        assert_eq!(items.len(), 1);
    }

    #[test]
    fn submit_review_advances_the_due_date_and_increments_reps() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        let now = Utc::now();

        submit_review_inner(
            &conn,
            SubmitReviewInput {
                card_id: card.id.clone(),
                direction: Direction::FaceOneToTwo,
                rating: Rating::Good,
            },
        )
        .unwrap();

        let after = db::review_state::get(&conn, &card.id, Direction::FaceOneToTwo)
            .unwrap()
            .unwrap();
        assert!(after.due > now);
        assert_eq!(after.reps, 1);
        assert_ne!(after.state, State::New);
    }

    #[test]
    fn submit_review_writes_a_review_log_entry() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        submit_review_inner(
            &conn,
            SubmitReviewInput {
                card_id: card.id.clone(),
                direction: Direction::FaceOneToTwo,
                rating: Rating::Easy,
            },
        )
        .unwrap();

        let (rating, count): (String, i64) = conn
            .query_row(
                "SELECT rating, COUNT(*) FROM review_log WHERE card_id = ?1",
                [&card.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(rating, "Easy");
    }

    #[test]
    fn submit_review_returns_not_found_for_a_direction_that_was_never_seeded() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        let err = submit_review_inner(
            &conn,
            SubmitReviewInput {
                card_id: card.id,
                direction: Direction::FaceTwoToOne,
                rating: Rating::Good,
            },
        )
        .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn select_new_items_introduces_lower_levels_before_higher_ones() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let lvl2 = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 2);
        let lvl1 = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        let items = select_new_items(&conn, &deck.id, Utc::now(), None).unwrap();

        let card_ids: Vec<&str> = items.iter().map(|(id, _)| id.as_str()).collect();
        let lvl1_pos = card_ids.iter().position(|id| *id == lvl1.id).unwrap();
        let lvl2_pos = card_ids.iter().position(|id| *id == lvl2.id).unwrap();
        assert!(lvl1_pos < lvl2_pos);
    }

    #[test]
    fn select_new_items_stops_at_the_daily_cap_counting_cards_not_items() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        for _ in 0..3 {
            new_card(&conn, &deck.id, Direction::all().to_vec(), 1);
        }

        let items = select_new_items(&conn, &deck.id, Utc::now(), Some(2)).unwrap();

        let distinct_cards: std::collections::HashSet<&str> =
            items.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(distinct_cards.len(), 2);
        assert_eq!(items.len(), 4); // 2 cards x 2 directions each
    }

    #[test]
    fn select_new_items_excludes_a_still_new_card_once_the_cap_is_already_spent_today() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let already_introduced = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        submit_review_inner(
            &conn,
            SubmitReviewInput {
                card_id: already_introduced.id,
                direction: Direction::FaceOneToTwo,
                rating: Rating::Good,
            },
        )
        .unwrap();
        new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        let items = select_new_items(&conn, &deck.id, Utc::now(), Some(1)).unwrap();

        assert!(items.is_empty());
    }

    #[test]
    fn reset_card_progress_reseeds_the_card_as_new_but_keeps_review_log() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        submit_review_inner(
            &conn,
            SubmitReviewInput {
                card_id: card.id.clone(),
                direction: Direction::FaceOneToTwo,
                rating: Rating::Good,
            },
        )
        .unwrap();
        assert_ne!(
            db::review_state::get(&conn, &card.id, Direction::FaceOneToTwo)
                .unwrap()
                .unwrap()
                .state,
            State::New
        );

        reset_card_progress_inner(&conn, &card.id).unwrap();

        let after = db::review_state::get(&conn, &card.id, Direction::FaceOneToTwo)
            .unwrap()
            .unwrap();
        assert_eq!(after.state, State::New);
        assert_eq!(after.reps, 0);

        let (log_count,): (i64,) = conn
            .query_row(
                "SELECT COUNT(*) FROM review_log WHERE card_id = ?1",
                [&card.id],
                |row| Ok((row.get(0)?,)),
            )
            .unwrap();
        assert_eq!(log_count, 1);

        let candidates = db::cards::new_card_candidates(&conn, &deck.id).unwrap();
        assert!(candidates.iter().any(|c| c.card_id == card.id));
    }

    #[test]
    fn reset_card_progress_returns_not_found_for_a_missing_card() {
        let conn = db::test_connection();

        let err = reset_card_progress_inner(&conn, "does-not-exist").unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }
}
