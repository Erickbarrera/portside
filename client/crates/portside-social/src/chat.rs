//! Direct messages through Bluesky's chat service (`chat.bsky.convo.*`, ADR-0021), reached
//! through the account host with `atproto-proxy`. Group conversations are listed but
//! Portside only starts one-to-one chats.

use portside_atproto::{Agent, BSKY_CHAT_PROXY};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Result, SocialError};
use crate::invite::{Invite, InviteLinks};
use crate::models::{Person, ProfileWire};
use crate::richtext::{MAX_MESSAGE_GRAPHEMES, facet_links, graphemes, link_facets};

const PROXY: Option<&str> = Some(BSKY_CHAT_PROXY);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Conversation {
    pub id: String,
    /// Changes whenever anything in the conversation does.
    pub rev: String,
    /// Everyone in it except you.
    pub members: Vec<Person>,
    pub last_message: Option<ChatMessage>,
    pub unread: u32,
    pub muted: bool,
    /// False for a message request you haven't accepted (replying accepts it).
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ChatMessage {
    pub id: String,
    pub rev: String,
    /// The sender's DID.
    pub sender: String,
    /// Empty for a deleted message.
    pub text: String,
    /// RFC 3339.
    pub sent_at: String,
    pub deleted: bool,
    /// Set when the message carries a Portside game invite.
    pub invite: Option<Invite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct MessagePage {
    /// Oldest first.
    pub messages: Vec<ChatMessage>,
    /// Pass back for older messages; None at the start of the conversation.
    pub cursor: Option<String>,
}

/// Something that changed since the last [`poll`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatEvent {
    Message {
        convo_id: String,
        message: ChatMessage,
    },
    Deleted {
        convo_id: String,
        message_id: String,
    },
    /// Read, accepted, left, muted…: reload the conversation list.
    Changed { convo_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogBatch {
    pub cursor: Option<String>,
    pub events: Vec<ChatEvent>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvoWire {
    id: String,
    #[serde(default)]
    rev: String,
    members: Vec<ProfileWire>,
    #[serde(default)]
    last_message: Option<Value>,
    #[serde(default)]
    muted: bool,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    unread_count: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageWire {
    #[serde(rename = "$type", default)]
    kind: String,
    id: String,
    #[serde(default)]
    rev: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    facets: Vec<Value>,
    #[serde(default)]
    sender: Option<SenderWire>,
    #[serde(default)]
    sent_at: String,
}

#[derive(Deserialize)]
struct SenderWire {
    did: String,
}

/// Turns chat service payloads into app types, recognising invites.
#[derive(Debug, Clone)]
pub struct Chat {
    invites: InviteLinks,
}

impl Chat {
    pub fn new(invites: InviteLinks) -> Chat {
        Chat { invites }
    }

    pub fn invites(&self) -> &InviteLinks {
        &self.invites
    }

    /// Your conversations, most recent first (accepted ones and requests).
    pub async fn conversations(&self, agent: &Agent) -> Result<Vec<Conversation>> {
        #[derive(Deserialize)]
        struct Page {
            convos: Vec<ConvoWire>,
        }
        let page: Page = agent
            .get(
                "chat.bsky.convo.listConvos",
                &[("limit", "50".to_owned())],
                PROXY,
            )
            .await?;
        Ok(page
            .convos
            .into_iter()
            .map(|c| self.convo(agent.did(), c))
            .collect())
    }

    /// The one-to-one conversation with `did`, created if needed. Fails with
    /// `NotFollowedBySender` or `MessagesDisabled` when their settings don't allow it.
    pub async fn conversation_with(&self, agent: &Agent, did: &str) -> Result<Conversation> {
        crate::graph::check_did(did)?;
        #[derive(Deserialize)]
        struct Found {
            convo: ConvoWire,
        }
        let found: Found = agent
            .get(
                "chat.bsky.convo.getConvoForMembers",
                &[("members", did.to_owned())],
                PROXY,
            )
            .await?;
        Ok(self.convo(agent.did(), found.convo))
    }

    pub async fn conversation(&self, agent: &Agent, convo_id: &str) -> Result<Conversation> {
        #[derive(Deserialize)]
        struct Found {
            convo: ConvoWire,
        }
        let found: Found = agent
            .get(
                "chat.bsky.convo.getConvo",
                &[("convoId", convo_id.to_owned())],
                PROXY,
            )
            .await?;
        Ok(self.convo(agent.did(), found.convo))
    }

    pub async fn messages(
        &self,
        agent: &Agent,
        convo_id: &str,
        cursor: Option<String>,
    ) -> Result<MessagePage> {
        #[derive(Deserialize)]
        struct Page {
            messages: Vec<Value>,
            #[serde(default)]
            cursor: Option<String>,
        }
        let mut query = vec![("convoId", convo_id.to_owned()), ("limit", "50".to_owned())];
        if let Some(c) = cursor {
            query.push(("cursor", c));
        }
        let page: Page = agent
            .get("chat.bsky.convo.getMessages", &query, PROXY)
            .await?;
        // The service sends newest first.
        let mut messages: Vec<ChatMessage> = page
            .messages
            .into_iter()
            .filter_map(|m| self.message(m))
            .collect();
        messages.reverse();
        Ok(MessagePage {
            messages,
            cursor: page.cursor,
        })
    }

    pub async fn send(&self, agent: &Agent, convo_id: &str, text: &str) -> Result<ChatMessage> {
        let text = text.trim();
        if text.is_empty() {
            return Err(SocialError::Invalid("The message is empty.".to_owned()));
        }
        if graphemes(text) > MAX_MESSAGE_GRAPHEMES {
            return Err(SocialError::Invalid(format!(
                "Messages can be up to {MAX_MESSAGE_GRAPHEMES} characters."
            )));
        }
        let mut message = json!({ "text": text });
        let facets = link_facets(text);
        if !facets.is_empty() {
            message["facets"] = Value::from(facets);
        }
        let sent: Value = agent
            .post(
                "chat.bsky.convo.sendMessage",
                &json!({ "convoId": convo_id, "message": message }),
                PROXY,
            )
            .await?;
        self.message(sent)
            .ok_or_else(|| SocialError::Invalid("the chat service returned no message".to_owned()))
    }

    /// Invites `did` to play: opens your conversation with them and sends the invite link.
    pub async fn invite(
        &self,
        agent: &Agent,
        did: &str,
        port_id: &str,
        title: &str,
    ) -> Result<(Conversation, ChatMessage)> {
        let convo = self.conversation_with(agent, did).await?;
        let message = self
            .send(agent, &convo.id, &self.invites.message(port_id, title))
            .await?;
        Ok((convo, message))
    }

    pub async fn mark_read(&self, agent: &Agent, convo_id: &str) -> Result<()> {
        let _: Value = agent
            .post(
                "chat.bsky.convo.updateRead",
                &json!({ "convoId": convo_id }),
                PROXY,
            )
            .await?;
        Ok(())
    }

    /// Accepts a message request.
    pub async fn accept(&self, agent: &Agent, convo_id: &str) -> Result<()> {
        let _: Value = agent
            .post(
                "chat.bsky.convo.acceptConvo",
                &json!({ "convoId": convo_id }),
                PROXY,
            )
            .await?;
        Ok(())
    }

    /// Changes since `cursor`. Call with None once to get a starting cursor.
    pub async fn poll(&self, agent: &Agent, cursor: Option<&str>) -> Result<LogBatch> {
        #[derive(Deserialize)]
        struct Log {
            #[serde(default)]
            cursor: Option<String>,
            #[serde(default)]
            logs: Vec<Value>,
        }
        let query: Vec<(&str, String)> = cursor
            .map(|c| vec![("cursor", c.to_owned())])
            .unwrap_or_default();
        let log: Log = agent.get("chat.bsky.convo.getLog", &query, PROXY).await?;
        let events = log
            .logs
            .into_iter()
            .filter_map(|entry| self.event(entry))
            .collect();
        Ok(LogBatch {
            cursor: log.cursor.or_else(|| cursor.map(str::to_owned)),
            events,
        })
    }

    fn event(&self, entry: Value) -> Option<ChatEvent> {
        let kind = entry.get("$type")?.as_str()?.to_owned();
        let convo_id = entry.get("convoId")?.as_str()?.to_owned();
        match kind.as_str() {
            "chat.bsky.convo.defs#logCreateMessage" => {
                let message = self.message(entry.get("message")?.clone())?;
                Some(ChatEvent::Message { convo_id, message })
            }
            "chat.bsky.convo.defs#logDeleteMessage" => {
                let message_id = entry.get("message")?.get("id")?.as_str()?.to_owned();
                Some(ChatEvent::Deleted {
                    convo_id,
                    message_id,
                })
            }
            _ => Some(ChatEvent::Changed { convo_id }),
        }
    }

    fn convo(&self, me: &str, c: ConvoWire) -> Conversation {
        Conversation {
            id: c.id,
            rev: c.rev,
            members: c
                .members
                .into_iter()
                .filter(|m| m.did != me)
                .map(Person::from)
                .collect(),
            last_message: c.last_message.and_then(|m| self.message(m)),
            unread: c.unread_count,
            muted: c.muted,
            accepted: c.status.as_deref() != Some("request"),
        }
    }

    fn message(&self, raw: Value) -> Option<ChatMessage> {
        let m: MessageWire = serde_json::from_value(raw).ok()?;
        let deleted = m.kind.ends_with("#deletedMessageView");
        if !(deleted || m.kind.is_empty() || m.kind.ends_with("#messageView")) {
            return None; // System messages (group changes) aren't shown.
        }
        let invite = (!deleted)
            .then(|| self.invites.find(&m.text, &facet_links(&m.facets)))
            .flatten();
        Some(ChatMessage {
            id: m.id,
            rev: m.rev,
            sender: m.sender.map(|s| s.did).unwrap_or_default(),
            text: if deleted { String::new() } else { m.text },
            sent_at: m.sent_at,
            deleted,
            invite,
        })
    }
}

/// The highest `rev` among conversations: where to start polling the log.
pub fn latest_rev(revs: impl IntoIterator<Item = String>) -> Option<String> {
    revs.into_iter().filter(|r| !r.is_empty()).max()
}
