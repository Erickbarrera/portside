//! Bluesky sign-in, friends, chat, invites and boards (client/docs/accounts-social.md).

use portside_social::Person;
use portside_social::StrongRef;
use portside_social::boards::{self, BoardPage, BoardQuery, SiteBoard, SubForum, ThreadView};
use portside_social::chat::{ChatMessage, Conversation, MessagePage};
use portside_social::graph;
use tauri::{AppHandle, State};

use crate::error::{CommandError, ErrorCode};
use crate::social::{self, Account};
use crate::state::AppState;

/// The signed-in account, if any.
#[tauri::command]
#[specta::specta]
pub fn auth_status(state: State<'_, AppState>) -> Option<Account> {
    state.social.account()
}

/// Opens the browser to sign in; resolves once the browser comes back. `account` is a
/// handle (any AT Protocol host) or empty for Bluesky.
#[tauri::command]
#[specta::specta]
pub async fn auth_sign_in(app: AppHandle, account: String) -> Result<Account, CommandError> {
    social::sign_in(&app, &account).await
}

/// Stops waiting for the browser.
#[tauri::command]
#[specta::specta]
pub fn auth_cancel(state: State<'_, AppState>) {
    state.social.cancel_sign_in();
}

#[tauri::command]
#[specta::specta]
pub async fn auth_sign_out(app: AppHandle) -> Result<(), CommandError> {
    social::sign_out(&app).await
}

/// People you follow who follow you back.
#[tauri::command]
#[specta::specta]
pub async fn friends_list(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Person>, CommandError> {
    let agent = state.social.agent()?;
    graph::friends(&agent)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

/// People who follow you that you don't follow back.
#[tauri::command]
#[specta::specta]
pub async fn friends_follows_you(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Person>, CommandError> {
    let agent = state.social.agent()?;
    graph::follows_you(&agent)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

#[tauri::command]
#[specta::specta]
pub async fn people_search(
    app: AppHandle,
    state: State<'_, AppState>,
    q: String,
) -> Result<Vec<Person>, CommandError> {
    let agent = state.social.agent()?;
    graph::search(&agent, &q)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

/// Follows someone on Bluesky; they're a friend once they follow back.
#[tauri::command]
#[specta::specta]
pub async fn friend_add(
    app: AppHandle,
    state: State<'_, AppState>,
    did: String,
) -> Result<Person, CommandError> {
    let agent = state.social.agent()?;
    graph::follow(&agent, &did)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

/// Unfollows someone on Bluesky.
#[tauri::command]
#[specta::specta]
pub async fn friend_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    did: String,
) -> Result<Person, CommandError> {
    let agent = state.social.agent()?;
    graph::unfollow(&agent, &did)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

#[tauri::command]
#[specta::specta]
pub async fn chat_conversations(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Conversation>, CommandError> {
    let agent = state.social.agent()?;
    state
        .social
        .chat
        .conversations(&agent)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

/// The conversation with someone, started if needed.
#[tauri::command]
#[specta::specta]
pub async fn chat_open(
    app: AppHandle,
    state: State<'_, AppState>,
    did: String,
) -> Result<Conversation, CommandError> {
    let agent = state.social.agent()?;
    state
        .social
        .chat
        .conversation_with(&agent, &did)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

#[tauri::command]
#[specta::specta]
pub async fn chat_get(
    app: AppHandle,
    state: State<'_, AppState>,
    convo_id: String,
) -> Result<Conversation, CommandError> {
    let agent = state.social.agent()?;
    state
        .social
        .chat
        .conversation(&agent, &convo_id)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

/// Messages, oldest first; pass the returned cursor for earlier ones.
#[tauri::command]
#[specta::specta]
pub async fn chat_messages(
    app: AppHandle,
    state: State<'_, AppState>,
    convo_id: String,
    cursor: Option<String>,
) -> Result<MessagePage, CommandError> {
    let agent = state.social.agent()?;
    state
        .social
        .chat
        .messages(&agent, &convo_id, cursor)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

#[tauri::command]
#[specta::specta]
pub async fn chat_send(
    app: AppHandle,
    state: State<'_, AppState>,
    convo_id: String,
    text: String,
) -> Result<ChatMessage, CommandError> {
    let agent = state.social.agent()?;
    state
        .social
        .chat
        .send(&agent, &convo_id, &text)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

#[tauri::command]
#[specta::specta]
pub async fn chat_mark_read(
    app: AppHandle,
    state: State<'_, AppState>,
    convo_id: String,
) -> Result<(), CommandError> {
    let agent = state.social.agent()?;
    state
        .social
        .chat
        .mark_read(&agent, &convo_id)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

/// Accepts a message request.
#[tauri::command]
#[specta::specta]
pub async fn chat_accept(
    app: AppHandle,
    state: State<'_, AppState>,
    convo_id: String,
) -> Result<(), CommandError> {
    let agent = state.social.agent()?;
    state
        .social
        .chat
        .accept(&agent, &convo_id)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

/// True while a chat is on screen: new messages are then checked every few seconds.
#[tauri::command]
#[specta::specta]
pub fn chat_watch(state: State<'_, AppState>, active: bool) {
    state.social.set_chat_active(active);
}

/// Sends a friend an invite to play a game: a direct message with its Portside link.
#[tauri::command]
#[specta::specta]
pub async fn game_invite(
    app: AppHandle,
    state: State<'_, AppState>,
    did: String,
    port_id: String,
) -> Result<Conversation, CommandError> {
    let agent = state.social.agent()?;
    let title = state
        .catalog
        .get(&port_id)?
        .map(|p| p.title)
        .ok_or_else(|| CommandError {
            code: ErrorCode::NotFound,
            message: "That game isn't in the catalog.".to_owned(),
        })?;
    state
        .social
        .chat
        .invite(&agent, &did, &port_id, &title)
        .await
        .map(|(convo, _)| convo)
        .map_err(|e| state.social.fail(&app, e))
}

/// Threads on a board, by latest activity or most popular.
#[tauri::command]
#[specta::specta]
pub async fn board_threads(
    app: AppHandle,
    state: State<'_, AppState>,
    query: BoardQuery,
) -> Result<BoardPage, CommandError> {
    let agent = state.social.agent()?;
    boards::board(&agent, &query)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

#[tauri::command]
#[specta::specta]
pub async fn board_thread(
    app: AppHandle,
    state: State<'_, AppState>,
    uri: String,
) -> Result<ThreadView, CommandError> {
    let agent = state.social.agent()?;
    boards::thread(&agent, &uri)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

/// Starts a thread on a game's board (a public Bluesky post).
#[tauri::command]
#[specta::specta]
pub async fn board_post(
    app: AppHandle,
    state: State<'_, AppState>,
    port_id: String,
    sub_forum: SubForum,
    title: String,
    body: String,
) -> Result<StrongRef, CommandError> {
    let agent = state.social.agent()?;
    boards::post_thread(&agent, &port_id, sub_forum, &title, &body)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

/// Starts a thread on Portside's own help or feature request board.
#[tauri::command]
#[specta::specta]
pub async fn board_post_site(
    app: AppHandle,
    state: State<'_, AppState>,
    board: SiteBoard,
    title: String,
    body: String,
) -> Result<StrongRef, CommandError> {
    let agent = state.social.agent()?;
    boards::post_site_thread(&agent, board, &title, &body)
        .await
        .map_err(|e| state.social.fail(&app, e))
}

/// Replies in a thread (a public Bluesky reply).
#[tauri::command]
#[specta::specta]
pub async fn board_reply(
    app: AppHandle,
    state: State<'_, AppState>,
    root: StrongRef,
    parent: StrongRef,
    tags: Vec<String>,
    text: String,
) -> Result<StrongRef, CommandError> {
    let agent = state.social.agent()?;
    boards::reply(&agent, &root, &parent, &tags, &text)
        .await
        .map_err(|e| state.social.fail(&app, e))
}
