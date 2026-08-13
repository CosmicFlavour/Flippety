use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    #[serde(rename = "1->2")]
    FaceOneToTwo,
    #[serde(rename = "2->1")]
    FaceTwoToOne,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::FaceOneToTwo => "1->2",
            Direction::FaceTwoToOne => "2->1",
        }
    }

    /// Inverse of `as_str`. Returns `None` for anything else (e.g. a
    /// corrupted row), so callers can skip rather than panic.
    pub fn parse(s: &str) -> Option<Direction> {
        match s {
            "1->2" => Some(Direction::FaceOneToTwo),
            "2->1" => Some(Direction::FaceTwoToOne),
            _ => None,
        }
    }

    pub fn all() -> [Direction; 2] {
        [Direction::FaceOneToTwo, Direction::FaceTwoToOne]
    }
}

pub fn default_directions() -> Vec<Direction> {
    Direction::all().to_vec()
}

/// New cards default to level 1, so manually-added cards surface soon;
/// curated decks (e.g. HSK1-9) set explicit levels to control ordering.
pub fn default_level() -> i64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardFull {
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub foot: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Card {
    pub id: String,
    pub deck_id: String,
    pub face_1: String,
    pub face_2: String,
    pub full: CardFull,
    pub tags: Vec<String>,
    pub directions: Vec<Direction>,
    pub level: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewCard {
    pub deck_id: String,
    pub face_1: String,
    pub face_2: String,
    pub full: CardFull,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_directions")]
    pub directions: Vec<Direction>,
    #[serde(default = "default_level")]
    pub level: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCard {
    pub id: String,
    pub face_1: String,
    pub face_2: String,
    pub full: CardFull,
    pub tags: Vec<String>,
    pub directions: Vec<Direction>,
    pub level: i64,
}
