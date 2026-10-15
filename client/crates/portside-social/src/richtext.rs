//! Links in post and message text, and splitting long text into 300-grapheme posts.

use serde_json::{Value, json};
use unicode_segmentation::UnicodeSegmentation;

/// Bluesky's limit per post and per message, in graphemes.
pub const MAX_GRAPHEMES: usize = 300;
/// Direct messages may be longer (1000 graphemes).
pub const MAX_MESSAGE_GRAPHEMES: usize = 1000;

pub fn graphemes(text: &str) -> usize {
    text.graphemes(true).count()
}

/// `http(s)://…` links in `text` with their byte ranges.
pub fn find_links(text: &str) -> Vec<(usize, usize, String)> {
    let mut links = Vec::new();
    let mut next_free = 0;
    for (start, _) in text.match_indices("http") {
        let rest = &text[start..];
        if start < next_free || !(rest.starts_with("https://") || rest.starts_with("http://")) {
            continue;
        }
        let mut end = start + rest.find(char::is_whitespace).unwrap_or(rest.len());
        while let Some(c) = text[start..end].chars().last()
            && ".,;:!?)\"'>]".contains(c)
        {
            end -= c.len_utf8();
        }
        let link = &text[start..end];
        if url::Url::parse(link).is_ok_and(|u| u.host_str().is_some()) {
            links.push((start, end, link.to_owned()));
            next_free = end;
        }
    }
    links
}

/// `app.bsky.richtext.facet` link facets for the links in `text`.
pub fn link_facets(text: &str) -> Vec<Value> {
    find_links(text)
        .into_iter()
        .map(|(start, end, uri)| {
            json!({
                "index": { "byteStart": start, "byteEnd": end },
                "features": [{ "$type": "app.bsky.richtext.facet#link", "uri": uri }]
            })
        })
        .collect()
}

/// Link targets named in facets (a client may shorten the visible text).
pub fn facet_links(facets: &[Value]) -> Vec<String> {
    facets
        .iter()
        .filter_map(|f| f.get("features")?.as_array())
        .flatten()
        .filter(|f| f.get("$type").and_then(Value::as_str) == Some("app.bsky.richtext.facet#link"))
        .filter_map(|f| f.get("uri")?.as_str().map(str::to_owned))
        .collect()
}

/// Splits text into posts of at most `limit` graphemes, preferring paragraph, then line,
/// then word breaks. Each part leaves room for a " (k/n)" marker.
pub fn split(text: &str, limit: usize) -> Vec<String> {
    let text = text.trim();
    if graphemes(text) <= limit {
        return vec![text.to_owned()];
    }
    let budget = limit.saturating_sub(10).max(1);
    let mut parts = Vec::new();
    let mut rest = text;
    while graphemes(rest) > budget {
        // Byte offset just past `budget` graphemes.
        let cut = rest
            .grapheme_indices(true)
            .nth(budget)
            .map_or(rest.len(), |(i, _)| i);
        let head = &rest[..cut];
        let at = head
            .rfind("\n\n")
            .filter(|&i| i > cut / 2)
            .or_else(|| head.rfind('\n').filter(|&i| i > cut / 2))
            .or_else(|| head.rfind(' ').filter(|&i| i > cut / 3))
            .unwrap_or(cut);
        parts.push(rest[..at].trim_end().to_owned());
        rest = rest[at..].trim_start();
    }
    if !rest.is_empty() {
        parts.push(rest.to_owned());
    }
    parts
}

/// Adds " (k/n)" to each part when there is more than one.
pub fn number_parts(parts: Vec<String>) -> Vec<String> {
    let n = parts.len();
    if n < 2 {
        return parts;
    }
    parts
        .into_iter()
        .enumerate()
        .map(|(i, p)| format!("{p} ({}/{n})", i + 1))
        .collect()
}

/// The "k" and "n" of a trailing " (k/n)" marker, and the text before it.
pub fn part_marker(text: &str) -> Option<(usize, usize, &str)> {
    let body = text.trim_end();
    let open = body.strip_suffix(')')?.rfind('(')?;
    let inner = &body[open + 1..body.len() - 1];
    let (k, n) = inner.split_once('/')?;
    let (k, n) = (k.parse().ok()?, n.parse().ok()?);
    (1 <= k && k <= n && n > 1).then(|| (k, n, body[..open].trim_end()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_and_byte_offsets() {
        let text = "héllo https://example.com/a?b=1. and (http://x.example)";
        let links = find_links(text);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].2, "https://example.com/a?b=1");
        assert_eq!(&text[links[0].0..links[0].1], "https://example.com/a?b=1");
        assert_eq!(links[1].2, "http://x.example");
        let facets = link_facets(text);
        assert_eq!(
            facet_links(&facets),
            vec![links[0].2.clone(), links[1].2.clone()]
        );
    }

    #[test]
    fn short_text_is_one_part() {
        assert_eq!(split("  hi  ", 300), vec!["hi"]);
        assert_eq!(number_parts(vec!["a".into()]), vec!["a"]);
    }

    #[test]
    fn long_text_splits_on_words_within_the_limit() {
        let text = "word ".repeat(200);
        let parts = number_parts(split(&text, MAX_GRAPHEMES));
        assert!(parts.len() >= 4);
        for p in &parts {
            assert!(graphemes(p) <= MAX_GRAPHEMES, "{} graphemes", graphemes(p));
            assert!(!p.starts_with(' '));
        }
        let (k, n, body) = part_marker(&parts[0]).expect("marker");
        assert_eq!((k, n), (1, parts.len()));
        assert!(body.ends_with("word"));
        let joined: Vec<&str> = parts.iter().map(|p| part_marker(p).expect("m").2).collect();
        assert_eq!(joined.join(" ").split_whitespace().count(), 200);
    }

    #[test]
    fn markers() {
        assert_eq!(part_marker("text (2/3)"), Some((2, 3, "text")));
        assert_eq!(part_marker("text (1/1)"), None);
        assert_eq!(part_marker("a (smile)"), None);
    }
}
