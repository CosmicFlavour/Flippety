use super::AppState;
use crate::db;
use crate::db::cards::NewCardCandidate;
use crate::error::{AppError, AppResult};
use crate::models::card::{CardFull, Direction, ExcludeItem};
use chrono::{DateTime, Utc};
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
/// *within* a level.
///
/// Cards are intentionally *not* sub-clustered by tag/theme within a level:
/// grouping similar material together ("blocked practice") is well-known to
/// feel easier in the moment but produce worse long-term retention than
/// mixing it up ("interleaving"), and in practice most decks have few enough
/// cards per tag that there was barely anything left to shuffle anyway.
fn ordered_new_candidates(
    conn: &Connection,
    deck_id: &str,
    now: DateTime<Utc>,
    exclude: Option<&ExcludeItem>,
    rng: &mut ThreadRng,
) -> AppResult<Vec<NewCardCandidate>> {
    let candidates = db::cards::new_card_candidates(conn, deck_id, now, exclude)?;

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

/// Decides which of a new candidate's directions to introduce now. A
/// single-direction candidate has nothing to stagger against. A
/// two-direction candidate introduces the deck's configured
/// `intro_direction` (or whichever direction is left, if that one was
/// already introduced/staggered on a previous call) and pushes the other
/// direction's `due` into the future, so both directions of the same new
/// card can't surface in the same sitting.
fn introduce_candidate(
    conn: &Connection,
    deck_id: &str,
    candidate: &NewCardCandidate,
    now: DateTime<Utc>,
) -> AppResult<Direction> {
    if candidate.directions.len() == 1 {
        return Ok(candidate.directions[0]);
    }

    let intro = db::decks::get(conn, deck_id)?
        .ok_or_else(|| AppError::NotFound(format!("deck {deck_id}")))?
        .intro_direction;
    let (chosen, sibling) = if candidate.directions.contains(&intro) {
        (
            intro,
            *candidate.directions.iter().find(|d| **d != intro).unwrap(),
        )
    } else {
        // intro_direction's direction isn't a candidate here (already
        // introduced/staggered previously) — introduce whichever's left.
        (candidate.directions[0], candidate.directions[1])
    };
    db::review_state::defer_new_direction(conn, &candidate.card_id, sibling, now)?;
    Ok(chosen)
}

/// The single next item to study right now for `deck_id` (or across all
/// decks if `None`): the most-overdue pending review if one exists,
/// otherwise the next new card to introduce (new-card introduction is
/// inherently per-deck, so this step is skipped when `deck_id` is `None` —
/// "everything due" mode has no UI yet), otherwise `None`.
///
/// `exclude` lets the caller ask for "the item after the one already on
/// screen": nothing in the DB changes between showing an item and
/// submitting its review, so without this, fetching one item ahead of an
/// unanswered one would deterministically return that same item again.
///
/// Deliberately fetches one item at a time rather than a batch: FSRS can
/// make an item due again within minutes (short-term learning steps), so
/// there's no stable "today's batch" to precompute — every call reflects
/// live review state.
fn get_next_study_item_inner(
    conn: &Connection,
    deck_id: Option<&str>,
    exclude: Option<ExcludeItem>,
) -> AppResult<Option<DueItem>> {
    let now = Utc::now();

    let due = db::review_state::due_review_items(conn, deck_id, now, exclude.as_ref())?;
    if let Some((card_id, direction)) = due.into_iter().next() {
        return Ok(Some(hydrate(conn, vec![(card_id, direction)])?.remove(0)));
    }

    let Some(deck_id) = deck_id else {
        return Ok(None);
    };

    let mut rng = rand::thread_rng();
    let candidates = ordered_new_candidates(conn, deck_id, now, exclude.as_ref(), &mut rng)?;
    let Some(candidate) = candidates.into_iter().next() else {
        return Ok(None);
    };

    let direction = introduce_candidate(conn, deck_id, &candidate, now)?;
    Ok(Some(
        hydrate(conn, vec![(candidate.card_id, direction)])?.remove(0),
    ))
}

#[tauri::command]
pub fn get_next_study_item(
    state: State<AppState>,
    deck_id: Option<String>,
    exclude: Option<ExcludeItem>,
) -> AppResult<Option<DueItem>> {
    let conn = state.conn();
    get_next_study_item_inner(&conn, deck_id.as_deref(), exclude)
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
    use chrono::Duration;
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

    /// Seeds a card+direction directly into the Review state with one rep
    /// and the given `due`, as if it had already been reviewed once.
    fn make_due_review(conn: &Connection, card_id: &str, direction: Direction, due: DateTime<Utc>) {
        let mut fsrs_card = crate::srs::new_card_state(due);
        fsrs_card.state = State::Review;
        fsrs_card.reps = 1;
        fsrs_card.due = due;
        db::review_state::upsert(conn, card_id, direction, &fsrs_card).unwrap();
    }

    #[test]
    fn get_next_study_item_prompts_face_1_for_the_1_to_2_direction() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        let item = get_next_study_item_inner(&conn, Some(&deck.id), None)
            .unwrap()
            .unwrap();

        assert_eq!(item.card_id, card.id);
        assert_eq!(item.direction, Direction::FaceOneToTwo);
        assert_eq!(item.prompt, "dog");
        assert_eq!(item.full.title, "狗");
    }

    #[test]
    fn get_next_study_item_prompts_face_2_for_the_2_to_1_direction() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        new_card(&conn, &deck.id, vec![Direction::FaceTwoToOne], 1);

        let item = get_next_study_item_inner(&conn, Some(&deck.id), None)
            .unwrap()
            .unwrap();

        assert_eq!(item.direction, Direction::FaceTwoToOne);
        assert_eq!(item.prompt, "狗");
    }

    #[test]
    fn get_next_study_item_filters_by_deck() {
        let conn = db::test_connection();
        let deck_a = new_deck(&conn, "Deck A");
        let deck_b = new_deck(&conn, "Deck B");
        let card_a = new_card(&conn, &deck_a.id, vec![Direction::FaceOneToTwo], 1);
        new_card(&conn, &deck_b.id, vec![Direction::FaceOneToTwo], 1);

        let item = get_next_study_item_inner(&conn, Some(&deck_a.id), None)
            .unwrap()
            .unwrap();

        assert_eq!(item.card_id, card_a.id);
    }

    #[test]
    fn get_next_study_item_returns_none_when_nothing_available() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");

        let item = get_next_study_item_inner(&conn, Some(&deck.id), None).unwrap();

        assert!(item.is_none());
    }

    #[test]
    fn get_next_study_item_returns_the_most_overdue_review() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let now = Utc::now();
        let less_overdue = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        make_due_review(
            &conn,
            &less_overdue.id,
            Direction::FaceOneToTwo,
            now - Duration::days(1),
        );
        let more_overdue = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        make_due_review(
            &conn,
            &more_overdue.id,
            Direction::FaceOneToTwo,
            now - Duration::days(5),
        );

        let item = get_next_study_item_inner(&conn, Some(&deck.id), None)
            .unwrap()
            .unwrap();

        assert_eq!(item.card_id, more_overdue.id);
    }

    #[test]
    fn get_next_study_item_returns_a_due_review_before_a_new_card() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let now = Utc::now();
        let reviewed = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        make_due_review(&conn, &reviewed.id, Direction::FaceOneToTwo, now);
        new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        let item = get_next_study_item_inner(&conn, Some(&deck.id), None)
            .unwrap()
            .unwrap();

        assert_eq!(item.card_id, reviewed.id);
    }

    #[test]
    fn get_next_study_item_introduces_lower_levels_before_higher_ones() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 2);
        let lvl1 = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        let item = get_next_study_item_inner(&conn, Some(&deck.id), None)
            .unwrap()
            .unwrap();

        assert_eq!(item.card_id, lvl1.id);
    }

    #[test]
    fn get_next_study_item_excludes_the_given_item_from_due_reviews() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let now = Utc::now();
        let more_overdue = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        make_due_review(
            &conn,
            &more_overdue.id,
            Direction::FaceOneToTwo,
            now - Duration::days(5),
        );
        let less_overdue = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        make_due_review(
            &conn,
            &less_overdue.id,
            Direction::FaceOneToTwo,
            now - Duration::days(1),
        );

        let exclude = ExcludeItem {
            card_id: more_overdue.id.clone(),
            direction: Direction::FaceOneToTwo,
        };
        let item = get_next_study_item_inner(&conn, Some(&deck.id), Some(exclude))
            .unwrap()
            .unwrap();

        assert_eq!(item.card_id, less_overdue.id);
    }

    #[test]
    fn get_next_study_item_returns_none_when_the_only_due_item_is_excluded() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        make_due_review(&conn, &card.id, Direction::FaceOneToTwo, Utc::now());

        let exclude = ExcludeItem {
            card_id: card.id,
            direction: Direction::FaceOneToTwo,
        };
        let item = get_next_study_item_inner(&conn, Some(&deck.id), Some(exclude)).unwrap();

        assert!(item.is_none());
    }

    /// This is the regression test for the bug the `exclude` parameter
    /// exists to prevent: fetching twice in a row without an intervening
    /// `submit_review` must not return the same not-yet-reviewed new card
    /// both times — nothing in the DB changes between those two calls
    /// otherwise, so the second call would deterministically re-offer the
    /// first call's result.
    #[test]
    fn get_next_study_item_excludes_a_new_card_already_shown_as_current() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        let first = get_next_study_item_inner(&conn, Some(&deck.id), None)
            .unwrap()
            .unwrap();
        let exclude = ExcludeItem {
            card_id: first.card_id,
            direction: first.direction,
        };
        let second = get_next_study_item_inner(&conn, Some(&deck.id), Some(exclude)).unwrap();

        assert!(second.is_none());
    }

    #[test]
    fn get_next_study_item_staggers_a_new_bidirectional_cards_second_direction() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, Direction::all().to_vec(), 1);

        let first = get_next_study_item_inner(&conn, Some(&deck.id), None)
            .unwrap()
            .unwrap();
        assert_eq!(first.direction, Direction::FaceOneToTwo); // deck default intro_direction

        let sibling_state = db::review_state::get(&conn, &card.id, Direction::FaceTwoToOne)
            .unwrap()
            .unwrap();
        assert_eq!(sibling_state.state, State::New);
        assert!(sibling_state.due > Utc::now());

        // Excluding the already-shown direction surfaces nothing else right
        // now — the sibling is staggered, not immediately offered.
        let exclude = ExcludeItem {
            card_id: card.id,
            direction: Direction::FaceOneToTwo,
        };
        let next = get_next_study_item_inner(&conn, Some(&deck.id), Some(exclude)).unwrap();
        assert!(next.is_none());
    }

    #[test]
    fn get_next_study_item_respects_the_decks_intro_direction_setting() {
        let conn = db::test_connection();
        let mut deck = new_deck(&conn, "Deck");
        deck.intro_direction = Direction::FaceTwoToOne;
        db::decks::update(&conn, &deck).unwrap();
        new_card(&conn, &deck.id, Direction::all().to_vec(), 1);

        let item = get_next_study_item_inner(&conn, Some(&deck.id), None)
            .unwrap()
            .unwrap();

        assert_eq!(item.direction, Direction::FaceTwoToOne);
    }

    #[test]
    fn get_next_study_item_does_not_stagger_a_single_direction_card() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        get_next_study_item_inner(&conn, Some(&deck.id), None)
            .unwrap()
            .unwrap();

        // No other direction was ever seeded for this card, so there's
        // nothing to have staggered — just a sanity check that introducing
        // a single-direction card doesn't error or touch anything odd.
        assert!(
            db::review_state::get(&conn, &card.id, Direction::FaceTwoToOne)
                .unwrap()
                .is_none()
        );
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

        let candidates = db::cards::new_card_candidates(&conn, &deck.id, Utc::now(), None).unwrap();
        assert!(candidates.iter().any(|c| c.card_id == card.id));
    }

    #[test]
    fn reset_card_progress_returns_not_found_for_a_missing_card() {
        let conn = db::test_connection();

        let err = reset_card_progress_inner(&conn, "does-not-exist").unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }
}
