use crate::error::AppResult;
use crate::models::deck::Deck;
use rusqlite::{params, Connection, OptionalExtension, Row};

fn map_row(row: &Row) -> rusqlite::Result<Deck> {
    Ok(Deck {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        new_cards_per_day: row.get("new_cards_per_day")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list(conn: &Connection) -> AppResult<Vec<Deck>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, new_cards_per_day, created_at, updated_at FROM decks ORDER BY name COLLATE NOCASE",
    )?;
    let decks = stmt
        .query_map([], map_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(decks)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Deck>> {
    conn.query_row(
        "SELECT id, name, description, new_cards_per_day, created_at, updated_at FROM decks WHERE id = ?1",
        params![id],
        map_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn insert(conn: &Connection, deck: &Deck) -> AppResult<()> {
    conn.execute(
        "INSERT INTO decks (id, name, description, new_cards_per_day, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            deck.id,
            deck.name,
            deck.description,
            deck.new_cards_per_day,
            deck.created_at,
            deck.updated_at
        ],
    )?;
    Ok(())
}

pub fn update(conn: &Connection, deck: &Deck) -> AppResult<bool> {
    let rows = conn.execute(
        "UPDATE decks SET name = ?1, description = ?2, new_cards_per_day = ?3, updated_at = ?4 WHERE id = ?5",
        params![
            deck.name,
            deck.description,
            deck.new_cards_per_day,
            deck.updated_at,
            deck.id
        ],
    )?;
    Ok(rows > 0)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<bool> {
    let rows = conn.execute("DELETE FROM decks WHERE id = ?1", params![id])?;
    Ok(rows > 0)
}
