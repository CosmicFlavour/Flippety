use chrono::{DateTime, Utc};
use rs_fsrs::{Card, Parameters, Rating, SchedulingInfo, FSRS};

fn scheduler() -> FSRS {
    FSRS::new(Parameters::default())
}

/// A freshly created card's FSRS state, as of `now`.
pub fn new_card_state(now: DateTime<Utc>) -> Card {
    Card {
        due: now,
        last_review: now,
        ..Card::default()
    }
}

/// Applies a self-rating to a card's current FSRS state, returning the
/// updated state and the log entry to persist.
pub fn review(card: Card, now: DateTime<Utc>, rating: Rating) -> SchedulingInfo {
    scheduler().next(card, now, rating)
}
