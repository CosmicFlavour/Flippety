use super::AppState;
use crate::db;
use crate::db::cards::NewCardCandidate;
use crate::error::{AppError, AppResult};
use crate::models::card::{CardFull, Direction};
use chrono::{DateTime, NaiveDate, Utc};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rs_fsrs::Rating;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
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

/// A lightweight (card_id, direction) reference into the queue — cheap to
/// return in bulk since it carries no card content, just enough to hydrate
/// on demand via `get_queue_cards`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueueRef {
    pub card_id: String,
    pub direction: Direction,
}

impl From<(String, Direction)> for QueueRef {
    fn from((card_id, direction): (String, Direction)) -> Self {
        QueueRef { card_id, direction }
    }
}

/// A study session's full manifest: which items are due for review, and
/// which new cards to introduce, each already shuffled. Split into two
/// blocks (rather than one combined list) so the frontend can show reviews
/// before new cards while still knowing the exact boundary between them —
/// e.g. to compute how many new cards a "bonus" fetch should skip.
#[derive(Debug, Clone, Serialize)]
pub struct QueueManifest {
    pub due: Vec<QueueRef>,
    pub new: Vec<QueueRef>,
}

/// Deterministically seeds an RNG from `(deck_id, day, salt)`. Used only
/// where two different calls need to agree on the same shuffle (e.g. the
/// capped and uncapped new-card selections, so a "bonus" fetch's result is a
/// clean superset of what the main session already showed) — a single
/// manifest fetch by itself doesn't need determinism, since it's cached
/// client-side for the rest of the session rather than re-queried.
fn seeded_rng(deck_id: Option<&str>, today: NaiveDate, salt: &str) -> StdRng {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    deck_id.hash(&mut hasher);
    today.hash(&mut hasher);
    salt.hash(&mut hasher);
    StdRng::seed_from_u64(hasher.finish())
}

/// Hydrates exactly the given (card_id, direction) refs with full card
/// content, preserving their order. Used to fetch a study session in
/// batches against a fixed, client-held manifest — unlike re-querying "what's
/// due" repeatedly, this can't skip or repeat items as review state changes
/// mid-session, since the set of refs to hydrate is already decided.
fn hydrate(conn: &Connection, refs: Vec<QueueRef>) -> AppResult<Vec<DueItem>> {
    let mut items = Vec::with_capacity(refs.len());
    for QueueRef { card_id, direction } in refs {
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

/// Picks which new cards to introduce for `deck_id`, in level-ascending
/// order, clustered by a shared tag within a level (so introduction reads as
/// themed blocks rather than a shuffle) and capped at `per_day_cap` distinct
/// cards already introduced today (`None` = unlimited). A card contributes
/// all of its still-New directions at once, so the cap counts cards, not items.
///
/// `per_day_cap` only truncates the *end* of the ordered list — it doesn't
/// affect how that order is built — so calling this twice with the same
/// `rng` seed but a smaller/larger cap yields results where one is a prefix
/// of the other. `get_bonus_new_cards` relies on this to continue seamlessly
/// past the cap without re-showing cards already seen in the capped call.
fn select_new_items(
    conn: &Connection,
    deck_id: &str,
    now: DateTime<Utc>,
    per_day_cap: Option<i64>,
    rng: &mut StdRng,
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

    let mut ordered: Vec<NewCardCandidate> = Vec::new();
    for group in level_groups {
        // Cluster by theme (the alphabetically-first tag; untagged cards
        // share a bucket), then shuffle both the theme order and each
        // theme's cards so introduction is randomized but still blocky. A
        // BTreeMap (not HashMap) keeps grouping itself deterministic, since
        // callers rely on the same seed reproducing the same order — a
        // HashMap's iteration order isn't a stability guarantee.
        let mut by_theme: BTreeMap<Option<String>, Vec<NewCardCandidate>> = BTreeMap::new();
        for candidate in group {
            let theme = candidate.tags.iter().min().cloned();
            by_theme.entry(theme).or_default().push(candidate);
        }
        let mut theme_groups: Vec<Vec<NewCardCandidate>> = by_theme.into_values().collect();
        theme_groups.shuffle(rng);
        for mut theme_group in theme_groups {
            theme_group.shuffle(rng);
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

/// The normal study session: every due review plus today's new-card
/// allowance (capped by the deck's `new_cards_per_day`), reviews before new
/// cards. Fetched as one unbounded manifest — no SQL/pagination cap on the
/// review count — so the frontend learns the true session size up front and
/// then hydrates full card content in batches against this fixed list.
fn get_due_queue_inner(conn: &Connection, deck_id: Option<&str>) -> AppResult<QueueManifest> {
    let now = Utc::now();
    let today = now.date_naive();

    let mut due = db::review_state::due_review_items(conn, deck_id, now)?;
    due.shuffle(&mut seeded_rng(deck_id, today, "due"));

    let new_items = match deck_id {
        Some(deck_id) => {
            let per_day_cap = db::decks::get(conn, deck_id)?
                .ok_or_else(|| AppError::NotFound(format!("deck {deck_id}")))?
                .new_cards_per_day;
            let mut rng = seeded_rng(Some(deck_id), today, "new");
            select_new_items(conn, deck_id, now, per_day_cap, &mut rng)?
        }
        // "Everything due" mode has no UI yet, so new-card leveling and the
        // daily cap (both inherently per-deck) don't apply here.
        None => {
            let mut items = db::review_state::due_new_items(conn, None, now, i64::MAX)?;
            items.shuffle(&mut seeded_rng(None, today, "new"));
            items
        }
    };

    Ok(QueueManifest {
        due: due.into_iter().map(QueueRef::from).collect(),
        new: new_items.into_iter().map(QueueRef::from).collect(),
    })
}

#[tauri::command]
pub fn get_due_queue(state: State<AppState>, deck_id: Option<String>) -> AppResult<QueueManifest> {
    let conn = state.conn();
    get_due_queue_inner(&conn, deck_id.as_deref())
}

/// The *same* deterministic new-card order used by `get_due_queue`'s `new`
/// block, but with the daily cap ignored — the full manifest, offered once a
/// session's normal queue is exhausted. Because it reuses the same seed, the
/// capped list from `get_due_queue` is guaranteed to be a prefix of this
/// one, so the frontend can just drop the cards it already showed (the first
/// `new.len()` entries) rather than needing any dedup logic.
fn get_bonus_new_cards_inner(conn: &Connection, deck_id: &str) -> AppResult<Vec<QueueRef>> {
    let now = Utc::now();
    let today = now.date_naive();
    let mut rng = seeded_rng(Some(deck_id), today, "new");
    let all_new = select_new_items(conn, deck_id, now, None, &mut rng)?;
    Ok(all_new.into_iter().map(QueueRef::from).collect())
}

#[tauri::command]
pub fn get_bonus_new_cards(state: State<AppState>, deck_id: String) -> AppResult<Vec<QueueRef>> {
    let conn = state.conn();
    get_bonus_new_cards_inner(&conn, &deck_id)
}

/// The "study ahead of schedule" bonus pool — reviews due within the next
/// `ahead_hours`, offered once a session's normal queue is exhausted. Always
/// disjoint from the main due block (`due > now` here vs `due <= now`
/// there), so there's nothing to dedup against it either.
fn get_ahead_reviews_inner(
    conn: &Connection,
    deck_id: &str,
    ahead_hours: i64,
) -> AppResult<Vec<QueueRef>> {
    let now = Utc::now();
    let today = now.date_naive();
    let until = now + chrono::Duration::hours(ahead_hours);
    let mut ahead = db::review_state::ahead_review_items(conn, Some(deck_id), now, until)?;
    ahead.shuffle(&mut seeded_rng(Some(deck_id), today, "ahead"));
    Ok(ahead.into_iter().map(QueueRef::from).collect())
}

#[tauri::command]
pub fn get_ahead_reviews(
    state: State<AppState>,
    deck_id: String,
    ahead_hours: i64,
) -> AppResult<Vec<QueueRef>> {
    let conn = state.conn();
    get_ahead_reviews_inner(&conn, &deck_id, ahead_hours)
}

#[tauri::command]
pub fn get_queue_cards(state: State<AppState>, refs: Vec<QueueRef>) -> AppResult<Vec<DueItem>> {
    let conn = state.conn();
    hydrate(&conn, refs)
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

        // `new_card` seeds it in the New state, so it surfaces as a new-card
        // introduction, not a due review.
        let manifest = get_due_queue_inner(&conn, None).unwrap();
        let items = hydrate(&conn, manifest.new).unwrap();

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

        let manifest = get_due_queue_inner(&conn, None).unwrap();
        let items = hydrate(&conn, manifest.new).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].direction, Direction::FaceTwoToOne);
        assert_eq!(items[0].prompt, "狗");
    }

    #[test]
    fn due_queue_returns_every_due_item_unbounded() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let now = Utc::now();
        for i in 0..30 {
            let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
            let mut fsrs_card = crate::srs::new_card_state(now);
            fsrs_card.state = State::Review;
            fsrs_card.reps = 1;
            fsrs_card.due = now - chrono::Duration::minutes(i);
            db::review_state::upsert(&conn, &card.id, Direction::FaceOneToTwo, &fsrs_card).unwrap();
        }

        let manifest = get_due_queue_inner(&conn, Some(&deck.id)).unwrap();

        assert_eq!(
            manifest.due.len(),
            30,
            "the manifest should list every due item, not a capped slice"
        );
    }

    #[test]
    fn due_queue_shuffles_deterministically_within_the_same_day() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let now = Utc::now();
        let mut due_order: Vec<String> = Vec::new();
        for i in 0..8 {
            let card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
            let mut fsrs_card = crate::srs::new_card_state(now);
            fsrs_card.state = State::Review;
            fsrs_card.reps = 1;
            fsrs_card.due = now - chrono::Duration::minutes(7 - i);
            db::review_state::upsert(&conn, &card.id, Direction::FaceOneToTwo, &fsrs_card).unwrap();
            due_order.push(card.id);
        }

        let first = get_due_queue_inner(&conn, Some(&deck.id)).unwrap();
        let second = get_due_queue_inner(&conn, Some(&deck.id)).unwrap();

        let first_order: Vec<&str> = first.due.iter().map(|r| r.card_id.as_str()).collect();
        let second_order: Vec<&str> = second.due.iter().map(|r| r.card_id.as_str()).collect();
        assert_eq!(
            first_order, second_order,
            "expected the same shuffle across repeated calls on the same day"
        );
        assert_ne!(
            first_order,
            due_order.iter().map(String::as_str).collect::<Vec<_>>(),
            "expected the due order to actually be shuffled, not just passed through"
        );
    }

    #[test]
    fn due_queue_filters_by_deck() {
        let conn = db::test_connection();
        let deck_a = new_deck(&conn, "Deck A");
        let deck_b = new_deck(&conn, "Deck B");
        new_card(&conn, &deck_a.id, vec![Direction::FaceOneToTwo], 1);
        new_card(&conn, &deck_b.id, vec![Direction::FaceOneToTwo], 1);

        let manifest = get_due_queue_inner(&conn, Some(&deck_a.id)).unwrap();

        assert_eq!(manifest.new.len(), 1);
    }

    #[test]
    fn due_queue_separates_the_due_and_new_blocks() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let now = Utc::now();
        let reviewed_card = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        let mut fsrs_card = crate::srs::new_card_state(now);
        fsrs_card.state = State::Review;
        fsrs_card.reps = 1;
        fsrs_card.due = now;
        db::review_state::upsert(
            &conn,
            &reviewed_card.id,
            Direction::FaceOneToTwo,
            &fsrs_card,
        )
        .unwrap();
        let new_card_ = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        let manifest = get_due_queue_inner(&conn, Some(&deck.id)).unwrap();

        assert_eq!(
            manifest.due,
            vec![QueueRef::from((reviewed_card.id, Direction::FaceOneToTwo))]
        );
        assert_eq!(
            manifest.new,
            vec![QueueRef::from((new_card_.id, Direction::FaceOneToTwo))]
        );
    }

    #[test]
    fn get_bonus_new_cards_continues_seamlessly_past_the_daily_cap() {
        let conn = db::test_connection();
        let mut deck = new_deck(&conn, "Deck");
        for _ in 0..5 {
            new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        }
        deck.new_cards_per_day = Some(2);
        db::decks::update(&conn, &deck).unwrap();

        let main = get_due_queue_inner(&conn, Some(&deck.id)).unwrap();
        assert_eq!(
            main.new.len(),
            2,
            "capped to the deck's daily new-card limit"
        );

        let bonus = get_bonus_new_cards_inner(&conn, &deck.id).unwrap();

        assert_eq!(
            bonus.len(),
            5,
            "the bonus pool reports every new-card candidate, uncapped"
        );
        assert_eq!(
            bonus[..main.new.len()],
            main.new,
            "the capped main block must be an exact prefix of the uncapped bonus list"
        );
    }

    #[test]
    fn get_ahead_reviews_only_returns_items_due_after_now() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let now = Utc::now();
        let overdue = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        let mut overdue_state = crate::srs::new_card_state(now);
        overdue_state.state = State::Review;
        overdue_state.reps = 1;
        overdue_state.due = now - chrono::Duration::hours(1);
        db::review_state::upsert(&conn, &overdue.id, Direction::FaceOneToTwo, &overdue_state)
            .unwrap();

        let ahead = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        let mut ahead_state = crate::srs::new_card_state(now);
        ahead_state.state = State::Review;
        ahead_state.reps = 1;
        ahead_state.due = now + chrono::Duration::hours(12);
        db::review_state::upsert(&conn, &ahead.id, Direction::FaceOneToTwo, &ahead_state).unwrap();

        let refs = get_ahead_reviews_inner(&conn, &deck.id, 24).unwrap();

        assert_eq!(
            refs,
            vec![QueueRef::from((ahead.id, Direction::FaceOneToTwo))]
        );
    }

    #[test]
    fn get_queue_cards_hydrates_exactly_the_given_refs_in_order() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let card_a = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);
        let card_b = new_card(&conn, &deck.id, vec![Direction::FaceTwoToOne], 1);

        let items = hydrate(
            &conn,
            vec![
                QueueRef::from((card_b.id.clone(), Direction::FaceTwoToOne)),
                QueueRef::from((card_a.id.clone(), Direction::FaceOneToTwo)),
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
    fn select_new_items_introduces_lower_levels_before_higher_ones() {
        let conn = db::test_connection();
        let deck = new_deck(&conn, "Deck");
        let lvl2 = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 2);
        let lvl1 = new_card(&conn, &deck.id, vec![Direction::FaceOneToTwo], 1);

        let mut rng = StdRng::seed_from_u64(0);
        let items = select_new_items(&conn, &deck.id, Utc::now(), None, &mut rng).unwrap();

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

        let mut rng = StdRng::seed_from_u64(0);
        let items = select_new_items(&conn, &deck.id, Utc::now(), Some(2), &mut rng).unwrap();

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

        let mut rng = StdRng::seed_from_u64(0);
        let items = select_new_items(&conn, &deck.id, Utc::now(), Some(1), &mut rng).unwrap();

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
