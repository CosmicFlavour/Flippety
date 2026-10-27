use crate::models::card::{default_intro_direction, Direction};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Deck {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Which direction of a new two-direction card is introduced first; the
    /// other direction is staggered (see `db::cards::new_card_candidates`).
    /// Moot for single-direction cards.
    pub intro_direction: Direction,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewDeck {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_intro_direction")]
    pub intro_direction: Direction,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateDeck {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_intro_direction")]
    pub intro_direction: Direction,
}
