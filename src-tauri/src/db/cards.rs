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

/// Optional narrowing for `browse`: `search` matches (case-insensitively,
/// ASCII only — SQLite's default `LIKE` behavior) against `title`; `tags`
/// requires every listed tag to be present on the card.
#[derive(Debug, Clone, Default)]
pub struct CardFilter {
    pub search: Option<String>,
    pub tags: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct CardPage {
    pub items: Vec<Card>,
    /// Total rows matching the filter, ignoring `limit`/`offset` — lets the
    /// caller know whether more pages exist.
    pub total: i64,
}

/// Escapes `%`, `_`, and the escape character itself so a filter value used
/// in a `LIKE ... ESCAPE '\'` clause is matched literally rather than as a
/// wildcard pattern.
fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Paginated, filtered listing for the card browser. Unlike `list_by_deck`
/// (used for full exports), this is meant for UI consumption: it applies
/// `search`/`tags` narrowing, then returns one page plus the total match
/// count so the caller can drive infinite-scroll/pagination.
pub fn browse(
    conn: &Connection,
    deck_id: &str,
    filter: &CardFilter,
    limit: i64,
    offset: i64,
) -> AppResult<CardPage> {
    let mut where_clauses = vec!["deck_id = ?".to_string()];
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(deck_id.to_string())];

    if let Some(search) = filter
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        where_clauses.push("title LIKE ? ESCAPE '\\'".to_string());
        params.push(Box::new(format!("%{}%", escape_like(search))));
    }

    for tag in &filter.tags {
        where_clauses.push("tags LIKE ? ESCAPE '\\'".to_string());
        params.push(Box::new(format!("%\"{}\"%", escape_like(tag))));
    }

    let where_sql = where_clauses.join(" AND ");
    let to_sql_params: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM cards WHERE {where_sql}"),
        to_sql_params.as_slice(),
        |row| row.get(0),
    )?;

    let mut page_params = to_sql_params;
    page_params.push(&limit);
    page_params.push(&offset);

    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM cards WHERE {where_sql} ORDER BY created_at LIMIT ? OFFSET ?"
    ))?;
    let items = stmt
        .query_map(page_params.as_slice(), map_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CardPage { items, total })
}

/// Distinct tags in use across a deck's cards, alphabetically sorted — feeds
/// the tag-filter UI in the card browser.
pub fn distinct_tags(conn: &Connection, deck_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT tags FROM cards WHERE deck_id = ?1")?;
    let mut tags = std::collections::BTreeSet::new();
    let rows = stmt.query_map(params![deck_id], |row| {
        row.get::<_, JsonColumn<Vec<String>>>(0)
    })?;
    for row in rows {
        tags.extend(row?.0);
    }
    Ok(tags.into_iter().collect())
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
/// with what's needed to order it for introduction: `level` picks the tier
/// (ascending), and `directions` lists only the directions that are still
/// `New`.
pub struct NewCardCandidate {
    pub card_id: String,
    pub level: i64,
    pub directions: Vec<Direction>,
}

/// New-card candidates for `deck_id`, one row per card, pre-sorted
/// `level ASC, created_at ASC`. Fine-grained shuffling happens in
/// `commands::study`.
pub fn new_card_candidates(conn: &Connection, deck_id: &str) -> AppResult<Vec<NewCardCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.level, rs.direction
         FROM cards c
         JOIN review_state rs ON rs.card_id = c.id
         WHERE c.deck_id = ?1 AND rs.state = 'New'
         ORDER BY c.level ASC, c.created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![deck_id], |row| {
            let direction: String = row.get("direction")?;
            Ok((
                row.get::<_, String>("id")?,
                row.get::<_, i64>("level")?,
                direction,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut candidates: Vec<NewCardCandidate> = Vec::new();
    for (card_id, level, direction) in rows {
        let Some(direction) = Direction::parse(&direction) else {
            continue;
        };
        match candidates.iter_mut().find(|c| c.card_id == card_id) {
            Some(existing) => existing.directions.push(direction),
            None => candidates.push(NewCardCandidate {
                card_id,
                level,
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

    fn seeded_deck(conn: &Connection) -> String {
        let deck = Deck {
            id: Uuid::new_v4().to_string(),
            name: "Test deck".into(),
            description: None,
            new_cards_per_day: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db::decks::insert(conn, &deck).unwrap();
        deck.id
    }

    fn insert_titled_card(conn: &Connection, deck_id: &str, title: &str, tags: &[&str]) -> Card {
        let now = Utc::now();
        let card = Card {
            id: Uuid::new_v4().to_string(),
            deck_id: deck_id.to_string(),
            face_1: "front".into(),
            face_2: "back".into(),
            full: CardFull {
                title: title.into(),
                subtitle: String::new(),
                body: String::new(),
                foot: String::new(),
            },
            tags: tags.iter().map(|t| t.to_string()).collect(),
            directions: vec![Direction::FaceOneToTwo],
            level: 1,
            created_at: now,
            updated_at: now,
        };
        insert(conn, &card).unwrap();
        card
    }

    #[test]
    fn browse_paginates_and_reports_total_across_the_full_match_set() {
        let conn = db::test_connection();
        let deck_id = seeded_deck(&conn);
        for i in 0..5 {
            insert_titled_card(&conn, &deck_id, &format!("card {i}"), &[]);
        }

        let page = browse(&conn, &deck_id, &CardFilter::default(), 2, 2).unwrap();

        assert_eq!(page.items.len(), 2);
        assert_eq!(page.total, 5);
    }

    #[test]
    fn browse_filters_by_title_search_case_insensitively() {
        let conn = db::test_connection();
        let deck_id = seeded_deck(&conn);
        insert_titled_card(&conn, &deck_id, "Chien", &[]);
        insert_titled_card(&conn, &deck_id, "Chat", &[]);

        let filter = CardFilter {
            search: Some("chi".into()),
            tags: vec![],
        };
        let page = browse(&conn, &deck_id, &filter, 10, 0).unwrap();

        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].full.title, "Chien");
    }

    #[test]
    fn browse_escapes_like_wildcards_in_the_search_term() {
        let conn = db::test_connection();
        let deck_id = seeded_deck(&conn);
        insert_titled_card(&conn, &deck_id, "100%", &[]);
        insert_titled_card(&conn, &deck_id, "100x", &[]);

        let filter = CardFilter {
            search: Some("100%".into()),
            tags: vec![],
        };
        let page = browse(&conn, &deck_id, &filter, 10, 0).unwrap();

        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].full.title, "100%");
    }

    #[test]
    fn browse_requires_every_filtered_tag_to_be_present() {
        let conn = db::test_connection();
        let deck_id = seeded_deck(&conn);
        insert_titled_card(&conn, &deck_id, "both", &["animals", "hsk1"]);
        insert_titled_card(&conn, &deck_id, "one", &["animals"]);

        let filter = CardFilter {
            search: None,
            tags: vec!["animals".into(), "hsk1".into()],
        };
        let page = browse(&conn, &deck_id, &filter, 10, 0).unwrap();

        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].full.title, "both");
    }

    #[test]
    fn browse_does_not_match_a_tag_that_is_only_a_substring_of_another_tag() {
        let conn = db::test_connection();
        let deck_id = seeded_deck(&conn);
        insert_titled_card(&conn, &deck_id, "categorized", &["category"]);
        insert_titled_card(&conn, &deck_id, "catlike", &["cat"]);

        let filter = CardFilter {
            search: None,
            tags: vec!["cat".into()],
        };
        let page = browse(&conn, &deck_id, &filter, 10, 0).unwrap();

        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].full.title, "catlike");
    }

    #[test]
    fn distinct_tags_are_deduplicated_and_sorted() {
        let conn = db::test_connection();
        let deck_id = seeded_deck(&conn);
        insert_titled_card(&conn, &deck_id, "a", &["hsk1", "animals"]);
        insert_titled_card(&conn, &deck_id, "b", &["animals", "food"]);
        insert_titled_card(&conn, &deck_id, "c", &[]);

        let tags = distinct_tags(&conn, &deck_id).unwrap();

        assert_eq!(tags, vec!["animals", "food", "hsk1"]);
    }

    #[test]
    fn browse_only_returns_cards_from_the_requested_deck() {
        let conn = db::test_connection();
        let deck_a = seeded_deck(&conn);
        let deck_b = seeded_deck(&conn);
        insert_titled_card(&conn, &deck_a, "in a", &[]);
        insert_titled_card(&conn, &deck_b, "in b", &[]);

        let page = browse(&conn, &deck_a, &CardFilter::default(), 10, 0).unwrap();

        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].full.title, "in a");
    }
}
