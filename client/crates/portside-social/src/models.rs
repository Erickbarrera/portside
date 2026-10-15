//! What the app shows. Wire shapes from the Bluesky lexicons are private to each module.

use serde::{Deserialize, Serialize};

/// Someone on the network, as seen by the signed-in account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Person {
    pub did: String,
    pub handle: String,
    pub display_name: Option<String>,
    /// Avatar image address on Bluesky's image host.
    pub avatar: Option<String>,
    /// You follow them.
    pub following: bool,
    /// They follow you. Both together make a friend.
    pub follows_you: bool,
}

impl Person {
    pub fn is_friend(&self) -> bool {
        self.following && self.follows_you
    }

    /// Display name, or the handle when there is none.
    pub fn name(&self) -> &str {
        self.display_name
            .as_deref()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or(&self.handle)
    }
}

/// `app.bsky.actor.defs#profileView*` and `chat.bsky.actor.defs#profileViewBasic`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileWire {
    pub did: String,
    pub handle: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub viewer: Option<ViewerWire>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewerWire {
    /// URI of your follow record.
    #[serde(default)]
    pub following: Option<String>,
    #[serde(default)]
    pub followed_by: Option<String>,
    #[serde(default)]
    pub blocking: Option<String>,
    #[serde(default)]
    pub blocked_by: Option<bool>,
    #[serde(default)]
    pub muted: Option<bool>,
}

impl From<ProfileWire> for Person {
    fn from(p: ProfileWire) -> Person {
        let viewer = p.viewer.unwrap_or_default();
        Person {
            did: p.did,
            handle: p.handle,
            display_name: p.display_name.filter(|n| !n.trim().is_empty()),
            avatar: p.avatar,
            following: viewer.following.is_some(),
            follows_you: viewer.followed_by.is_some(),
        }
    }
}

impl ProfileWire {
    /// Blocked either way or muted: kept out of friend lists and boards.
    pub(crate) fn hidden(&self) -> bool {
        self.viewer.as_ref().is_some_and(|v| {
            v.blocking.is_some() || v.blocked_by == Some(true) || v.muted == Some(true)
        })
    }
}

/// A record's address and version, as replies and likes need it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct StrongRef {
    pub uri: String,
    pub cid: String,
}

#[derive(Deserialize)]
pub(crate) struct CreatedRecord {
    pub uri: String,
    pub cid: String,
}

/// The record key at the end of an `at://did/collection/rkey` URI.
pub(crate) fn rkey(uri: &str) -> Option<&str> {
    let rest = uri.strip_prefix("at://")?;
    let mut parts = rest.splitn(3, '/');
    let (_, _, rkey) = (parts.next()?, parts.next()?, parts.next()?);
    (!rkey.is_empty() && !rkey.contains('/')).then_some(rkey)
}

pub(crate) fn now_rfc3339() -> String {
    // RFC 3339 in UTC without pulling in a date library.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rem / 3600, rem % 3600 / 60, rem % 60);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mo <= 2);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}.000Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rkeys() {
        assert_eq!(
            rkey("at://did:plc:a/app.bsky.graph.follow/3k2"),
            Some("3k2")
        );
        assert_eq!(rkey("at://did:plc:a/app.bsky.graph.follow"), None);
        assert_eq!(rkey("https://x"), None);
    }

    #[test]
    fn timestamps_look_like_rfc3339() {
        let t = now_rfc3339();
        assert_eq!(t.len(), 24);
        assert!(t.ends_with('Z') && t.as_bytes()[10] == b'T');
        assert!(t.starts_with("20"));
    }
}
