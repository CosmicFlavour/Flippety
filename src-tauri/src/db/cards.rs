use crate::error::AppResult;
use crate::models::card::{Card, CardFull, Direction};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::de::DeserializeOwned;

/// Wraps a column stored as a JSON string. `Row::get` deserializes it on
/// read, so malformed JSON surfaces as a real `rusqlite::Error` (and from
/// there `AppError::Db`) instead of silently falling back to a default.
struct JsonColumn<T>(T);

impl<T: DeserializeOwned> FromSql for JsonColumn<T> {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        serde_json::from_str(value.as_str()?)
            .map(JsonColumn)
            .map_err(|e| FromSqlError::Other(Box::new(e)))
    }
}

fn map_row(row: &Row) -> rusqlite::Result<Card> {
    Ok(Card {
        id: row.get("id")?,
        deck_id: row.get("deck_id")?,
        face_1: row.get("face_1")?,
        face_2: row.get("face_2")?,
        full: CardFull {
            title: row.get("title")?,
            subtitle: row.get("subtitle")?,
            body: row.get("body")?,
            foot: row.get("foot")?,
        },
        tags: row.get::<_, JsonColumn<Vec<String>>>("tags")?.0,
        directions: row.get::<_, JsonColumn<Vec<Direction>>>("directions")?.0,
        level: row.get("level")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

const SELECT_COLUMNS: &str = "id, deck_id, face_1, face_2, title, subtitle, body, foot, tags, directions, level, created_at, updated_at";

pub fn list_by_deck(conn: &Connection, deck_id: &str) -> AppResult<Vec<Card>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM cards WHERE deck_id = ?1 ORDER BY created_at"
    ))?;
    let cards = stmt
        .query_map(params![deck_id], map_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(cards)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Card>> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM cards WHERE id = ?1"),
        params![id],
        map_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn insert(conn: &Connection, card: &Card) -> AppResult<()> {
    conn.execute(
        "INSERT INTO cards (id, deck_id, face_1, face_2, title, subtitle, body, foot, tags, directions, level, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            card.id,
            card.deck_id,
            card.face_1,
            card.face_2,
            card.full.title,
            card.full.subtitle,
            card.full.body,
            card.full.foot,
            serde_json::to_string(&card.tags)?,
            serde_json::to_string(&card.directions)?,
            card.level,
            card.created_at,
            card.updated_at,
        ],
    )?;
    Ok(())
}

pub fn update(conn: &Connection, card: &Card) -> AppResult<bool> {
    let rows = conn.execute(
        "UPDATE cards SET face_1 = ?1, face_2 = ?2, title = ?3, subtitle = ?4, body = ?5, foot = ?6,
         tags = ?7, directions = ?8, level = ?9, updated_at = ?10 WHERE id = ?11",
        params![
            card.face_1,
            card.face_2,
            card.full.title,
            card.full.subtitle,
            card.full.body,
            card.full.foot,
            serde_json::to_string(&card.tags)?,
            serde_json::to_string(&card.directions)?,
            card.level,
            card.updated_at,
            card.id,
        ],
    )?;
    Ok(rows > 0)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<bool> {
    let rows = conn.execute("DELETE FROM cards WHERE id = ?1", params![id])?;
    Ok(rows > 0)
}

/// A card still awaiting its first review in at least one direction, along
/// with what's needed to order/cluster it for introduction: `level` picks
/// the tier (ascending), `tags` drive theme clustering within a tier, and
/// `directions` lists only the directions that are still `New`.
pub struct NewCardCandidate {
    pub card_id: String,
    pub level: i64,
    pub tags: Vec<String>,
    pub directions: Vec<Direction>,
}

/// New-card candidates for `deck_id`, one row per card, pre-sorted
/// `level ASC, created_at ASC`. Fine-grained shuffling/clustering happens in
/// `commands::study`.
pub fn new_card_candidates(conn: &Connection, deck_id: &str) -> AppResult<Vec<NewCardCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.level, c.tags, rs.direction
         FROM cards c
         JOIN review_state rs ON rs.card_id = c.id
         WHERE c.deck_id = ?1 AND rs.state = 'New'
         ORDER BY c.level ASC, c.created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![deck_id], |row| {
            let tags: JsonColumn<Vec<String>> = row.get("tags")?;
            let direction: String = row.get("direction")?;
            Ok((
                row.get::<_, String>("id")?,
                row.get::<_, i64>("level")?,
                tags.0,
                direction,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut candidates: Vec<NewCardCandidate> = Vec::new();
    for (card_id, level, tags, direction) in rows {
        let Some(direction) = Direction::parse(&direction) else {
            continue;
        };
        match candidates.iter_mut().find(|c| c.card_id == card_id) {
            Some(existing) => existing.directions.push(direction),
            None => candidates.push(NewCardCandidate {
                card_id,
                level,
                tags,
                directions: vec![direction],
            }),
        }
    }
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::error::AppError;
    use crate::models::deck::Deck;
    use chrono::Utc;
    use uuid::Uuid;

    /// A deck plus a raw INSERT that bypasses `insert()` so a column can hold
    /// JSON that would never occur through normal app use.
    fn card_with_raw_column(conn: &Connection, column: &str, raw_value: &str) -> String {
        let deck = Deck {
            id: Uuid::new_v4().to_string(),
            name: "Test deck".into(),
            description: None,
            new_cards_per_day: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db::decks::insert(conn, &deck).unwrap();

        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let (tags, directions) = match column {
            "tags" => (raw_value, "[\"1->2\"]"),
            "directions" => ("[]", raw_value),
            other => panic!("unexpected column {other}"),
        };
        conn.execute(
            "INSERT INTO cards (id, deck_id, face_1, face_2, title, subtitle, body, foot, tags, directions, created_at, updated_at)
             VALUES (?1, ?2, 'dog', '狗', '', '', '', '', ?3, ?4, ?5, ?5)",
            params![id, deck.id, tags, directions, now],
        )
        .unwrap();
        id
    }

    #[test]
    fn get_surfaces_an_error_for_malformed_tags_json_instead_of_defaulting() {
        let conn = db::test_connection();
        let id = card_with_raw_column(&conn, "tags", "not valid json");

        let err = get(&conn, &id).unwrap_err();
        assert!(matches!(err, AppError::Db(_)));
    }

    #[test]
    fn get_surfaces_an_error_for_malformed_directions_json_instead_of_defaulting() {
        let conn = db::test_connection();
        let id = card_with_raw_column(&conn, "directions", "not valid json");

        let err = get(&conn, &id).unwrap_err();
        assert!(matches!(err, AppError::Db(_)));
    }
}
