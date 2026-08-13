use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Deck {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Cap on how many new cards may be introduced per day. `None` = unlimited.
    pub new_cards_per_day: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewDeck {
    pub name: String,
    pub description: Option<String>,
    pub new_cards_per_day: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateDeck {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub new_cards_per_day: Option<i64>,
}
