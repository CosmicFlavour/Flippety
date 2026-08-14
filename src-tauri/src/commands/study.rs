use super::AppState;
use crate::db;
use crate::db::cards::NewCardCandidate;
use crate::error::{AppError, AppResult};
use crate::models::card::{CardFull, Direction};
use chrono::Utc;
use rand::rngs::ThreadRng;
use rand::seq::SliceRandom;
use rs_fsrs::Rating;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
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

/// A batch of items to study right now, fetched fresh against live review
/// state — see `get_study_batch_inner` for why "study session" isn't a
/// fixed, precomputed set here.
#[derive(Debug, Clone, Serialize)]
pub struct StudyBatch {
    pub items: Vec<DueItem>,
    /// New-card items that exist beyond today's cap and so aren't in
    /// `items` — lets the frontend offer "pull more new cards" once the cap
    /// is reached, and size that offer.
    pub bonus_new_available: i64,
}

fn hydrate(conn: &Connection, refs: Vec<(String, Direction)>) -> AppResult<Vec<DueItem>> {
    let mut items = Vec::with_capacity(refs.len());
    for (card_id, direction) in refs {
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

/// Every card in `deck_id` still awaiting its first review in at least one
/// direction, in the order they'd be introduced: level-ascending (levels are
/// a deliberate curriculum sequence, e.g. HSK1 before HSK2), fully shuffled
/// *within* a level. No daily-cap truncation here — that's applied by the
/// caller, since how much of this list is available depends on what's
/// already been introduced today.
///
/// Cards are intentionally *not* sub-clustered by tag/theme within a level:
/// grouping similar material together ("blocked practice") is well-known to
/// feel easier in the moment but produce worse long-term retention than
/// mixing it up ("interleaving"), and in practice most decks have few enough
/// cards per tag that there was barely anything left to shuffle anyway.
fn ordered_new_candidates(
    conn: &Connection,
    deck_id: &str,
    rng: &mut ThreadRng,
) -> AppResult<Vec<NewCardCandidate>> {
    let candidates = db::cards::new_card_candidates(conn, deck_id)?;

    // Candidates arrive level-ascending; group consecutive same-level runs.
    let mut level_groups: Vec<Vec<NewCardCandidate>> = Vec::new();
    for candidate in candidates {
        match level_groups.last_mut() {
            Some(group) if group[0].level == candidate.level => group.push(candidate),
            _ => level_groups.push(vec![candidate]),
        }
    }

    let mut ordered: Vec<NewCardCandidate> = Vec::new();
    for mut group in level_groups {
        group.shuffle(rng);
        ordered.extend(group);
    }
    Ok(ordered)
}

/// Takes candidate cards from the front of `candidates`, expanding each into
/// its still-New directions, until at least `min_items` items have been
/// collected. The card that crosses the threshold is included in full, so
/// the result can overflow `min_items` by up to one card's worth of
/// directions — cards are never split across that boundary, since once
/// introduced the two directions are independently scheduled items anyway,
/// so there's nothing to gain from forcing them into the same batch.
fn take_new_items(candidates: &[NewCardCandidate], min_items: usize) -> Vec<(String, Direction)> {
    let mut items = Vec::new();
    for candidate in candidates {
        if items.len() >= min_items {
            break;
        }
        for direction in &candidate.directions {
            items.push((candidate.card_id.clone(), *direction));
        }
    }
    items
}

/// The next batch of up to `limit` items to study right now: most-overdue
/// reviews first, remaining slots filled with new cards (capped by the
/// deck's `new_cards_per_day` unless `bypass_new_card_cap`), then the whole
/// batch shuffled together — so a new card's two directions, or a review and
/// a new card, don't land in a predictable order.
///
/// This is deliberately *not* a fixed "today's session" fetched once:
/// FSRS reschedules items on short-term learning steps that can be due again
/// within minutes, so there's no such thing as a stable daily set to
/// precompute — every call reflects live review state, and the caller is
/// expected to call this again (not reuse an old result) once its buffer
/// runs low.
fn get_study_batch_inner(
    conn: &Connection,
    deck_id: Option<&str>,
    limit: i64,
    bypass_new_card_cap: bool,
) -> AppResult<StudyBatch> {
    let now = Utc::now();
    let limit = limit.max(0) as usize;

    let mut due = db::review_state::due_review_items(conn, deck_id, now)?;
    due.truncate(limit);
    let remaining_slots = limit.saturating_sub(due.len());

    let mut rng = rand::thread_rng();
    let (new_items, bonus_new_available) = match deck_id {
        Some(deck_id) => {
            let candidates = ordered_new_candidates(conn, deck_id, &mut rng)?;
            let per_day_cap = db::decks::get(conn, deck_id)?
                .ok_or_else(|| AppError::NotFound(format!("deck {deck_id}")))?
                .new_cards_per_day;

            let cards_under_cap = match per_day_cap {
                Some(cap) => {
                    let introduced_today =
                        db::review_state::new_cards_introduced_today(conn, deck_id, now)?;
                    ((cap - introduced_today).max(0) as usize).min(candidates.len())
                }
                None => candidates.len(),
            };
            let (under_cap, beyond_cap) = candidates.split_at(cards_under_cap);

            let bonus_new_available: i64 = if per_day_cap.is_some() {
                beyond_cap.iter().map(|c| c.directions.len() as i64).sum()
            } else {
                0
            };

            let pool: &[NewCardCandidate] = if bypass_new_card_cap {
                &candidates
            } else {
                under_cap
            };
            (take_new_items(pool, remaining_slots), bonus_new_available)
        }
        // "Everything due" mode has no UI yet, so new-card introduction
        // (inherently per-deck) doesn't apply here.
        None => (Vec::new(), 0),
    };

    let mut combined = due;
    combined.extend(new_items);
    combined.shuffle(&mut rng);

    let items = hydrate(conn, combined)?;
    Ok(StudyBatch {
        items,
        bonus_new_available,
    })
}

#[tauri::command]
pub fn get_study_batch(
    state: State<AppState>,
    deck_id: Option<String>,
    limit: i64,
    bypass_new_card_cap: bool,
) -> AppResult<StudyBatch> {
    let conn = state.conn();
    get_study_batch_inner(&conn, deck_id.as_deref(), limit, bypass_new_card_cap)
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
    fn get_study_batch_prompts_face_1_for_the_1_to_2_direction() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        let batch = get_study_batch_inner(&conn, Some(&deck.id), 10, false).unwrap();

        assert_eq!(batch.items.len(), 1);
        assert_eq!(batch.items[0].card_id, card.id);
        assert_eq!(batch.items[0].direction, Direction::FaceOneToTwo);
        assert_eq!(batch.items[0].prompt, "dog");
        assert_eq!(batch.items[0].full.title, "狗");
    }

    #[test]
    fn get_study_batch_prompts_face_2_for_the_2_to_1_direction() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        new_card(&conn, &deck.id, vec![Direction::FaceTwoToOne], 1);

        let batch = get_study_batch_inner(&conn, Some(&deck.id), 10, false).unwrap();

        assert_eq!(batch.items.len(), 1);
        assert_eq!(batch.items[0].direction, Direction::FaceTwoToOne);
        assert_eq!(batch.items[0].prompt, "狗");
    }

    #[test]
    fn get_study_batch_filters_by_deck() {
        let conn = db::test_connection();
        let deck_a = new_deck(&conn, "Deck A");
        let deck_b = new_deck(&conn, "Deck B");
        new_card(&conn, &deck_a.id, vec![Direction::FaceOneToTwo], 1);
        new_card(&conn, &deck_b.id, vec![Direction::FaceOneToTwo], 1);

        let batch = get_study_batch_inner(&conn, Some(&deck_a.id), 10, false).unwrap();

        assert_eq!(batch.items.len(), 1);
    }

    #[test]
    fn get_study_batch_prioritizes_the_most_overdue_reviews_first() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let now = Utc::now();
        let mut cards = Vec::new();
        for i in 0..5 {
            let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
            let mut fsrs_card = crate::srs::new_card_state(now);
            fsrs_card.state = State::Review;
            fsrs_card.reps = 1;
            fsrs_card.due = now - chrono::Duration::days(i + 1);
            db::review_state::upsert(&conn, &card.id, Direction::FaceOneToTwo, &fsrs_card).unwrap();
            cards.push(card);
        }
        // cards[4] is due -5 days (most overdue) ... cards[0] is due -1 day (least overdue).

        let batch = get_study_batch_inner(&conn, Some(&deck.id), 3, false).unwrap();

        let ids: std::collections::HashSet<&str> =
            batch.items.iter().map(|i| i.card_id.as_str()).collect();
        assert_eq!(ids.len(), 3);
        for most_overdue in &cards[2..5] {
            assert!(ids.contains(most_overdue.id.as_str()));
        }
        for least_overdue in &cards[0..2] {
            assert!(!ids.contains(least_overdue.id.as_str()));
        }
    }

    #[test]
    fn get_study_batch_fills_remaining_slots_with_new_cards_after_due_reviews() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let now = Utc::now();
        for _ in 0..2 {
            let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
            let mut fsrs_card = crate::srs::new_card_state(now);
            fsrs_card.state = State::Review;
            fsrs_card.reps = 1;
            fsrs_card.due = now;
            db::review_state::upsert(&conn, &card.id, Direction::FaceOneToTwo, &fsrs_card).unwrap();
        }
        for _ in 0..5 {
            new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        }

        let batch = get_study_batch_inner(&conn, Some(&deck.id), 4, false).unwrap();

        assert_eq!(
            batch.items.len(),
            4,
            "2 due reviews plus 2 new cards to fill the remaining slots"
        );
    }

    #[test]
    fn get_study_batch_overflows_the_limit_rather_than_splitting_a_cards_directions() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        for _ in 0..3 {
            new_card(&conn, &deck.id, Direction::all().to_vec(), 1);
        }

        let batch = get_study_batch_inner(&conn, Some(&deck.id), 3, false).unwrap();

        assert_eq!(
            batch.items.len(),
            4,
            "the 2nd bidirectional card's both directions land in the batch, one more than the limit of 3"
        );
        let distinct_cards: std::collections::HashSet<&str> =
            batch.items.iter().map(|i| i.card_id.as_str()).collect();
        assert_eq!(distinct_cards.len(), 2);
    }

    #[test]
    fn get_study_batch_caps_new_cards_at_the_daily_limit_counting_cards_not_items() {
        let conn = db::test_connection();
        let mut deck = new_deck(&conn, "Deck");
        for _ in 0..3 {
            new_card(&conn, &deck.id, Direction::all().to_vec(), 1);
        }
        deck.new_cards_per_day = Some(2);
        db::decks::update(&conn, &deck).unwrap();

        let batch = get_study_batch_inner(&conn, Some(&deck.id), 100, false).unwrap();

        let distinct_cards: std::collections::HashSet<&str> =
            batch.items.iter().map(|i| i.card_id.as_str()).collect();
        assert_eq!(distinct_cards.len(), 2);
        assert_eq!(batch.items.len(), 4); // 2 cards x 2 directions each
    }

    #[test]
    fn get_study_batch_excludes_new_cards_once_the_cap_is_already_spent_today_and_reports_the_bonus_pool(
    ) {
        let conn = db::test_connection();
        let mut deck = new_deck(&conn, "Deck");
        deck.new_cards_per_day = Some(1);
        db::decks::update(&conn, &deck).unwrap();
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

        let batch = get_study_batch_inner(&conn, Some(&deck.id), 10, false).unwrap();

        assert!(
            batch.items.is_empty(),
            "the just-introduced card is in a short-term Learning step, not due yet, \
             and the 2nd new card is beyond today's cap of 1"
        );
        assert_eq!(batch.bonus_new_available, 1);
    }

    #[test]
    fn get_study_batch_bypasses_the_cap_when_requested() {
        let conn = db::test_connection();
        let mut deck = new_deck(&conn, "Deck");
        deck.new_cards_per_day = Some(1);
        db::decks::update(&conn, &deck).unwrap();
        for _ in 0..3 {
            new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        }

        let capped = get_study_batch_inner(&conn, Some(&deck.id), 10, false).unwrap();
        assert_eq!(capped.items.len(), 1);

        let bypassed = get_study_batch_inner(&conn, Some(&deck.id), 10, true).unwrap();
        assert_eq!(bypassed.items.len(), 3);
    }

    #[test]
    fn ordered_new_candidates_introduces_lower_levels_before_higher_ones() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let lvl2 = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 2);
        let lvl1 = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        let mut rng = rand::thread_rng();
        let ordered = ordered_new_candidates(&conn, &deck.id, &mut rng).unwrap();

        let ids: Vec<&str> = ordered.iter().map(|c| c.card_id.as_str()).collect();
        let lvl1_pos = ids.iter().position(|id| *id == lvl1.id).unwrap();
        let lvl2_pos = ids.iter().position(|id| *id == lvl2.id).unwrap();
        assert!(lvl1_pos < lvl2_pos);
    }

    #[test]
    fn hydrate_preserves_the_given_order() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card_a = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        let card_b = new_card(&conn, &deck.id, vec![Direction::FaceTwoToOne], 1);

        let items = hydrate(
            &conn,
            vec![
                (card_b.id.clone(), Direction::FaceTwoToOne),
                (card_a.id.clone(), Direction::FaceOneToTwo),
            ],
        )
        .unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].card_id, card_b.id);
        assert_eq!(items[1].card_id, card_a.id);
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
