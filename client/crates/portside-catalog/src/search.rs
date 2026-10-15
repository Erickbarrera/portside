//! Local search over the catalog copy: filters by OS, status and type; text matches titles
//! only (summaries carry no description).

use portside_api::SearchParams;
use portside_api::models::{Page, PortSummary};
use rusqlite::types::Value;

use crate::error::CatalogError;
use crate::store::Catalog;

const DEFAULT_LIMIT: u32 = 50;

impl Catalog {
    /// Searches the local copy.
    pub fn search(&self, params: &SearchParams) -> Result<Page<PortSummary>, CatalogError> {
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, 100);
        let offset: u64 = match &params.cursor {
            Some(c) => c.parse().map_err(|_| CatalogError::Cursor)?,
            None => 0,
        };
        let fts = params.q.as_deref().and_then(fts_query);

        let mut sql = String::from("SELECT p.summary FROM ports p");
        let mut args: Vec<Value> = Vec::new();
        let mut filters: Vec<&str> = Vec::new();
        if let Some(q) = &fts {
            sql.push_str(" JOIN ports_fts f ON f.rowid = p.rowid");
            filters.push("ports_fts MATCH ?");
            args.push(Value::Text(q.clone()));
        }
        if let Some(status) = params.status {
            filters.push("p.status = ?");
            args.push(Value::Text(status.as_str().into()));
        }
        if let Some(t) = params.port_type {
            filters.push("p.type = ?");
            args.push(Value::Text(t.as_str().into()));
        }
        if let Some(os) = params.os {
            filters.push("EXISTS (SELECT 1 FROM port_os o WHERE o.port_id = p.id AND o.os = ?)");
            args.push(Value::Text(os.as_str().into()));
        }
        if !filters.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&filters.join(" AND "));
        }
        // Text relevance first, then popularity (stars only break ties: the bundled SQLite
        // has no math functions to blend them).
        sql.push_str(if fts.is_some() {
            " ORDER BY bm25(ports_fts), p.stars IS NULL, p.stars DESC, p.id"
        } else {
            " ORDER BY p.stars IS NULL, p.stars DESC, p.id"
        });
        sql.push_str(" LIMIT ? OFFSET ?");
        args.push(Value::Integer(i64::from(limit) + 1));
        args.push(Value::Integer(
            i64::try_from(offset).map_err(|_| CatalogError::Cursor)?,
        ));

        let conn = self.conn();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args), |r| r.get::<_, String>(0))?;
        let mut items = Vec::new();
        for json in rows {
            items.push(serde_json::from_str::<PortSummary>(&json?)?);
        }
        let more = items.len() > limit as usize;
        items.truncate(limit as usize);
        Ok(Page {
            items,
            cursor: more.then(|| (offset + u64::from(limit)).to_string()),
        })
    }
}

/// Turns user text into an FTS5 query: every word must prefix-match a title word.
/// Quotes keep FTS syntax characters in the input from being interpreted.
fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"*", w.to_lowercase()))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

#[cfg(test)]
mod tests {
    use super::fts_query;

    #[test]
    fn builds_prefix_queries() {
        assert_eq!(
            fts_query("Mario  Kart").as_deref(),
            Some("\"mario\"* \"kart\"*")
        );
        assert_eq!(
            fts_query("a\"b OR c*").as_deref(),
            Some("\"a\"* \"b\"* \"or\"* \"c\"*")
        );
        assert_eq!(fts_query("  -- "), None);
    }
}
