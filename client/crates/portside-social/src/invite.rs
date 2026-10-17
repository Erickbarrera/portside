//! Game invites (ADR-0021): a direct message with a link to the game on the Portside
//! website. Any chat app shows the link; Portside recognises it and shows an invite card.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::richtext::find_links;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Invite {
    pub port_id: String,
}

/// Builds and recognises invite links for one website, e.g.
/// `https://erickbarrera.github.io/portside/`.
#[derive(Debug, Clone)]
pub struct InviteLinks {
    page: Url,
}

impl InviteLinks {
    pub fn new(site: &str) -> Option<InviteLinks> {
        let mut site = Url::parse(site).ok()?;
        if !site.path().ends_with('/') {
            let path = format!("{}/", site.path());
            site.set_path(&path);
        }
        Some(InviteLinks {
            page: site.join("catalog/").ok()?,
        })
    }

    /// `{site}/catalog/?q={title}&port={id}`: the website's catalog searched for the game.
    pub fn link(&self, port_id: &str, title: &str) -> Url {
        let mut url = self.page.clone();
        url.query_pairs_mut()
            .append_pair("q", title)
            .append_pair("port", port_id);
        url
    }

    pub fn message(&self, port_id: &str, title: &str) -> String {
        format!(
            "🎮 Want to play {title}? Get it on Portside: {}",
            self.link(port_id, title)
        )
    }

    /// The invite in a message, from its link facets or the links in its text.
    pub fn find(&self, text: &str, facet_links: &[String]) -> Option<Invite> {
        facet_links
            .iter()
            .cloned()
            .chain(find_links(text).into_iter().map(|l| l.2))
            .find_map(|link| self.parse(&link))
    }

    fn parse(&self, link: &str) -> Option<Invite> {
        let url = Url::parse(link).ok()?;
        let same_page = url.origin() == self.page.origin()
            && url.path().trim_end_matches('/') == self.page.path().trim_end_matches('/');
        if !same_page {
            return None;
        }
        let port_id = url
            .query_pairs()
            .find(|(k, _)| k == "port")
            .map(|(_, v)| v.into_owned())?;
        valid_port_id(&port_id).then_some(Invite { port_id })
    }
}

fn valid_port_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && id.len() <= 200
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let links = InviteLinks::new("https://example.github.io/portside-catalog").expect("links");
        let text = links.message("example-port", "Example Game (PC Port)");
        assert!(text.contains(
            "https://example.github.io/portside-catalog/catalog/?q=Example+Game+%28PC+Port%29&port=example-port"
        ));
        assert_eq!(
            links.find(&text, &[]),
            Some(Invite {
                port_id: "example-port".into()
            })
        );
    }

    #[test]
    fn other_sites_and_bad_ids_are_ignored() {
        let links = InviteLinks::new("https://example.github.io/portside-catalog/").expect("l");
        assert_eq!(
            links.find("https://evil.example/portside-catalog/catalog/?port=x", &[]),
            None
        );
        assert_eq!(
            links.find(
                "https://example.github.io/portside-catalog/catalog/?port=..%2F..",
                &[]
            ),
            None
        );
        let facet = vec!["https://example.github.io/portside-catalog/catalog?port=a1".to_owned()];
        assert_eq!(
            links.find("shortened link", &facet),
            Some(Invite {
                port_id: "a1".into()
            })
        );
    }
}
