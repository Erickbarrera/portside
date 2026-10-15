//! Portside's social features on Bluesky (ADR-0016, ADR-0021): friends (mutual follows),
//! direct messages and game invites through Bluesky's chat service, and discussion boards
//! made of tagged posts (docs/specs/boards.md, docs/specs/friends-chat.md). No Tauri here.

pub mod boards;
pub mod chat;
mod error;
pub mod graph;
pub mod invite;
mod models;
pub mod richtext;

pub use crate::error::{Result, SocialError};
pub use crate::models::{Person, StrongRef};
