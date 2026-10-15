//! The Bluesky account and the social features built on it (ADR-0016, ADR-0021): sign-in,
//! friends, direct messages (polled in the background and sent to the UI as
//! `chat-changed`), game invites and boards.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};
use std::time::Duration;

use portside_atproto::oauth::DEFAULT_SCOPE;
use portside_atproto::{
    Agent, AtprotoError, CallbackListener, ClientConfig, OAuthClient, Resolver, SessionStore,
};
use portside_social::chat::{Chat, ChatEvent};
use portside_social::graph;
use portside_social::invite::InviteLinks;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use tauri_specta::Event;
use tokio::sync::Notify;

use crate::error::{CommandError, ErrorCode};
use crate::keychain::KeychainStore;
use crate::state::AppState;

/// How long the browser has to come back from the sign-in page.
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Checking for new messages while a chat is on screen, and otherwise.
const CHAT_POLL_ACTIVE: Duration = Duration::from_secs(4);
const CHAT_POLL_IDLE: Duration = Duration::from_secs(45);

/// The signed-in Bluesky account.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Account {
    pub did: String,
    pub handle: String,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    /// The sign-in allows direct messages (older sign-ins may not).
    pub can_chat: bool,
}

/// Sent when someone signs in or out, or the sign-in expires.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct AuthChanged {
    pub account: Option<Account>,
}

/// New messages and other conversation changes.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ChatChanged {
    pub events: Vec<ChatEvent>,
}

pub struct Social {
    http: reqwest::Client,
    oauth: OAuthClient,
    store: Arc<dyn SessionStore>,
    agent: RwLock<Option<Arc<Agent>>>,
    account: Mutex<Option<Account>>,
    sign_in_cancel: Mutex<Option<Arc<Notify>>>,
    pub chat: Chat,
    chat_cursor: tokio::sync::Mutex<Option<String>>,
    chat_active: AtomicBool,
    chat_wake: Notify,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Social {
    /// `site` is the Portside website, where invite links point.
    pub fn new(user_agent: &str, site: &str) -> anyhow::Result<Social> {
        let http = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()?;
        let oauth = OAuthClient::new(
            http.clone(),
            Resolver::new(http.clone())?,
            ClientConfig::loopback(DEFAULT_SCOPE)?,
        );
        let invites = InviteLinks::new(site)
            .ok_or_else(|| anyhow::anyhow!("{site} can't be used for invite links"))?;
        Ok(Social {
            http,
            oauth,
            store: Arc::new(KeychainStore),
            agent: RwLock::new(None),
            account: Mutex::new(None),
            sign_in_cancel: Mutex::new(None),
            chat: Chat::new(invites),
            chat_cursor: tokio::sync::Mutex::new(None),
            chat_active: AtomicBool::new(false),
            chat_wake: Notify::new(),
        })
    }

    pub fn account(&self) -> Option<Account> {
        lock(&self.account).clone()
    }

    /// The signed-in account's connection, or `E_SIGNED_OUT`.
    pub fn agent(&self) -> Result<Arc<Agent>, CommandError> {
        self.agent
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| CommandError {
                code: ErrorCode::SignedOut,
                message: "Sign in with Bluesky to use this.".to_owned(),
            })
    }

    /// Tells the poller whether a chat is on screen (polls often) or not.
    pub fn set_chat_active(&self, active: bool) {
        if !self.chat_active.swap(active, Ordering::Relaxed) && active {
            self.chat_wake.notify_one();
        }
    }

    pub fn cancel_sign_in(&self) {
        if let Some(cancel) = lock(&self.sign_in_cancel).take() {
            cancel.notify_one();
        }
    }

    fn set_signed_in(&self, app: &AppHandle, agent: Arc<Agent>, account: Account) {
        *self.agent.write().unwrap_or_else(PoisonError::into_inner) = Some(agent);
        *lock(&self.account) = Some(account.clone());
        let _ = AuthChanged {
            account: Some(account),
        }
        .emit(app);
        self.chat_wake.notify_one();
    }

    fn set_signed_out(&self, app: &AppHandle) {
        *self.agent.write().unwrap_or_else(PoisonError::into_inner) = None;
        *lock(&self.account) = None;
        let _ = AuthChanged { account: None }.emit(app);
    }

    /// Turns a failed call into a command error; an expired sign-in signs out.
    pub fn fail(&self, app: &AppHandle, e: impl Into<SocialFailure>) -> CommandError {
        let e: SocialFailure = e.into();
        if e.is_expired() {
            let _ = self.store.clear();
            self.set_signed_out(app);
        }
        e.into()
    }
}

/// Either crate's error, so commands can use `?` on both.
pub enum SocialFailure {
    Atproto(AtprotoError),
    Social(portside_social::SocialError),
}

impl SocialFailure {
    fn is_expired(&self) -> bool {
        matches!(
            self,
            SocialFailure::Atproto(AtprotoError::SessionExpired)
                | SocialFailure::Social(portside_social::SocialError::Atproto(
                    AtprotoError::SessionExpired
                ))
        )
    }
}

impl From<AtprotoError> for SocialFailure {
    fn from(e: AtprotoError) -> Self {
        SocialFailure::Atproto(e)
    }
}

impl From<portside_social::SocialError> for SocialFailure {
    fn from(e: portside_social::SocialError) -> Self {
        SocialFailure::Social(e)
    }
}

impl From<SocialFailure> for CommandError {
    fn from(e: SocialFailure) -> Self {
        match e {
            SocialFailure::Atproto(e) => e.into(),
            SocialFailure::Social(e) => e.into(),
        }
    }
}

fn account_from(
    session: &portside_atproto::Session,
    profile: Option<portside_social::Person>,
) -> Account {
    let handle = profile
        .as_ref()
        .map(|p| p.handle.clone())
        .or_else(|| session.handle.clone())
        .unwrap_or_else(|| session.did.clone());
    Account {
        did: session.did.clone(),
        handle,
        display_name: profile.as_ref().and_then(|p| p.display_name.clone()),
        avatar: profile.and_then(|p| p.avatar),
        can_chat: session.has_scope("transition:chat.bsky"),
    }
}

/// Signs in again with the session saved in the keychain, if any. Runs at start.
pub fn spawn_restore(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let social = &state.social;
        let store = social.store.clone();
        let loaded = tokio::task::spawn_blocking(move || store.load()).await;
        let session = match loaded {
            Ok(Ok(Some(session))) => session,
            Ok(Ok(None)) => return,
            Ok(Err(e)) => {
                eprintln!("could not read the saved Bluesky sign-in: {e}");
                return;
            }
            Err(_) => return,
        };
        let agent = match Agent::new(social.http.clone(), session.clone(), social.store.clone()) {
            Ok(agent) => Arc::new(agent),
            Err(e) => {
                eprintln!("the saved Bluesky sign-in is unusable: {e}");
                let _ = social.store.clear();
                return;
            }
        };
        // Signed in straight away (offline too); the profile fills in when it arrives.
        social.set_signed_in(&app, agent.clone(), account_from(&session, None));
        match graph::profile(&agent, &session.did).await {
            Ok(profile) => {
                let session = agent.session().await;
                social.set_signed_in(&app, agent, account_from(&session, Some(profile)));
            }
            Err(e) => {
                let _ = social.fail(&app, e);
            }
        }
    });
}

/// Signs in through the browser. `account` is a handle, or empty for Bluesky.
pub async fn sign_in(app: &AppHandle, account: &str) -> Result<Account, CommandError> {
    let state = app.state::<AppState>();
    let social = &state.social;
    social.cancel_sign_in();
    let cancel = Arc::new(Notify::new());
    *lock(&social.sign_in_cancel) = Some(cancel.clone());

    let listener = CallbackListener::bind(&social.oauth.config().redirect_uri)
        .await
        .map_err(CommandError::from)?;
    let pending = social
        .oauth
        .begin(account, listener.redirect_uri())
        .await
        .map_err(CommandError::from)?;
    app.opener()
        .open_url(pending.authorize_url.as_str(), None::<&str>)
        .map_err(|e| CommandError {
            code: ErrorCode::Auth,
            message: format!("Couldn't open your browser: {e}"),
        })?;
    let params = tokio::select! {
        params = listener.wait(SIGN_IN_TIMEOUT) => params.map_err(CommandError::from)?,
        () = cancel.notified() => {
            return Err(CommandError {
                code: ErrorCode::Cancelled,
                message: "Sign-in was cancelled.".to_owned(),
            });
        }
    };
    lock(&social.sign_in_cancel).take();
    let session = social
        .oauth
        .complete(pending, params)
        .await
        .map_err(CommandError::from)?;
    let store = social.store.clone();
    let to_save = session.clone();
    tokio::task::spawn_blocking(move || store.save(&to_save))
        .await
        .map_err(|e| CommandError {
            code: ErrorCode::Keychain,
            message: e.to_string(),
        })?
        .map_err(CommandError::from)?;
    let agent = Arc::new(
        Agent::new(social.http.clone(), session.clone(), social.store.clone())
            .map_err(CommandError::from)?,
    );
    let profile = graph::profile(&agent, &session.did).await.ok();
    let account = account_from(&session, profile);
    *social.chat_cursor.lock().await = None;
    social.set_signed_in(app, agent, account.clone());
    Ok(account)
}

pub async fn sign_out(app: &AppHandle) -> Result<(), CommandError> {
    let state = app.state::<AppState>();
    let social = &state.social;
    if let Ok(agent) = social.agent() {
        let session = agent.session().await;
        // Best effort: the device forgets the sign-in either way.
        let _ = portside_atproto::oauth::revoke(&social.http, &session).await;
    }
    let store = social.store.clone();
    let cleared = tokio::task::spawn_blocking(move || store.clear()).await;
    social.set_signed_out(app);
    *social.chat_cursor.lock().await = None;
    match cleared {
        Ok(result) => result.map_err(CommandError::from),
        Err(e) => Err(CommandError {
            code: ErrorCode::Keychain,
            message: e.to_string(),
        }),
    }
}

/// Polls Bluesky's message log while signed in and sends changes to the UI.
pub fn spawn_chat_poller(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let state = app.state::<AppState>();
            let social = &state.social;
            let wait = if social.chat_active.load(Ordering::Relaxed) {
                CHAT_POLL_ACTIVE
            } else {
                CHAT_POLL_IDLE
            };
            let can_chat = social.account().is_some_and(|a| a.can_chat);
            if let (true, Ok(agent)) = (can_chat, social.agent()) {
                poll_once(&app, &agent).await;
            }
            tokio::select! {
                () = tokio::time::sleep(wait) => {}
                () = social.chat_wake.notified() => {}
            }
        }
    });
}

async fn poll_once(app: &AppHandle, agent: &Agent) {
    let state = app.state::<AppState>();
    let social = &state.social;
    let mut cursor = social.chat_cursor.lock().await;
    if cursor.is_none() {
        // Start from the newest conversation change, so old history isn't replayed.
        match social.chat.conversations(agent).await {
            Ok(convos) => {
                *cursor = portside_social::chat::latest_rev(convos.into_iter().map(|c| c.rev));
                if cursor.is_none() {
                    *cursor = social
                        .chat
                        .poll(agent, None)
                        .await
                        .ok()
                        .and_then(|b| b.cursor);
                }
            }
            Err(e) => {
                let _ = social.fail(app, e);
            }
        }
        return;
    }
    match social.chat.poll(agent, cursor.as_deref()).await {
        Ok(batch) => {
            *cursor = batch.cursor;
            if !batch.events.is_empty() {
                let _ = ChatChanged {
                    events: batch.events,
                }
                .emit(app);
            }
        }
        Err(e) => {
            let _ = social.fail(app, e);
        }
    }
}
