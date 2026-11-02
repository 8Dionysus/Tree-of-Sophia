//! Search over a public research index. The index hands each item's id, kind and canonical
//! texts once; this rule owns what a query means: its words, case, matching and the chosen items.
use serde_json::Value;

/// The longest part of a record's canonical text that a query reads.
const RECORD_CHARS: usize = 5000;
/// The most items one search returns.
const MAX_LIMIT: usize = 100;
const DEFAULT_LIMIT: usize = 30;

pub struct ResearchIndexSearch {
    items: Vec<Entry>,
}

struct Entry {
    id: String,
    kind: String,
    text: String,
}

impl ResearchIndexSearch {
    /// `documents` is `[{id, kind, title, record}]`: `title` and `record` are the index's canonical
    /// JSON texts of the item's title and record, in index order.
    pub fn new(documents: &[u8]) -> Result<Self, String> {
        let value: Value = serde_json::from_slice(documents).map_err(|e| e.to_string())?;
        let rows = value
            .as_array()
            .ok_or("research index documents must be an array")?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let field = |name: &str| row.get(name).and_then(Value::as_str).unwrap_or("");
            let id = row
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or("every research index document needs an id")?;
            let record: String = field("record").chars().take(RECORD_CHARS).collect();
            let text = format!("{id} {} {record}", field("title")).to_lowercase();
            items.push(Entry {
                id: id.to_owned(),
                kind: field("kind").to_owned(),
                text,
            });
        }
        Ok(Self { items })
    }

    /// `query` is `{query, limit?, kind?}`; the answer is the ids of the items whose text holds every
    /// word of the query, ignoring case, in index order.
    pub fn search(&self, query: &[u8]) -> Result<Vec<String>, String> {
        let value: Value = serde_json::from_slice(query).map_err(|e| e.to_string())?;
        let words: Vec<String> = value
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let kind = value
            .get("kind")
            .and_then(Value::as_str)
            .filter(|kind| !kind.is_empty());
        let limit = value
            .get("limit")
            .and_then(Value::as_u64)
            .map_or(DEFAULT_LIMIT, |limit| limit as usize)
            .min(MAX_LIMIT);
        Ok(self
            .items
            .iter()
            .filter(|item| kind.is_none_or(|kind| item.kind == kind))
            .filter(|item| words.iter().all(|word| item.text.contains(word.as_str())))
            .take(limit)
            .map(|item| item.id.clone())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::ResearchIndexSearch;

    fn index() -> ResearchIndexSearch {
        ResearchIndexSearch::new(
            r#"[
              {"id":"overman","kind":"concept","title":"{\"en\":\"Overman\",\"ru\":\"Сверхчеловек\"}","record":"{}"},
              {"id":"rope","kind":"figure","title":"{\"en\":\"The tightrope walker\",\"ru\":\"Канатный плясун\"}","record":"{\"note\":\"over the market\"}"},
              {"id":"child","kind":"concept","title":"{\"en\":\"Child\"}","record":"{}"}
            ]"#.as_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn every_word_matches_ignoring_case_in_index_order() {
        let index = index();
        assert_eq!(
            index.search(r#"{"query":"OVER"}"#.as_bytes()).unwrap(),
            ["overman", "rope"]
        );
        assert_eq!(
            index
                .search(r#"{"query":"сверхЧЕЛОВЕК"}"#.as_bytes())
                .unwrap(),
            ["overman"]
        );
        assert_eq!(
            index
                .search(r#"{"query":"tightrope market"}"#.as_bytes())
                .unwrap(),
            ["rope"]
        );
        assert!(
            index
                .search(r#"{"query":"tightrope child"}"#.as_bytes())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn kind_and_limit_select_the_items() {
        let index = index();
        assert_eq!(
            index
                .search(r#"{"query":"over","kind":"figure"}"#.as_bytes())
                .unwrap(),
            ["rope"]
        );
        assert_eq!(
            index
                .search(r#"{"query":"over","limit":1}"#.as_bytes())
                .unwrap(),
            ["overman"]
        );
        assert_eq!(
            index
                .search(r#"{"query":"","limit":1000}"#.as_bytes())
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn a_record_is_read_up_to_its_bound() {
        let far = format!("{}needle", "x".repeat(5000));
        let documents = serde_json::to_vec(&serde_json::json!([
            {"id":"far","kind":"passage","title":"","record":far}
        ]))
        .unwrap();
        let index = ResearchIndexSearch::new(&documents).unwrap();
        assert!(
            index
                .search(r#"{"query":"needle"}"#.as_bytes())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_document_without_an_id_is_refused() {
        assert!(ResearchIndexSearch::new(r#"[{"kind":"concept"}]"#.as_bytes()).is_err());
    }
}
