//! Jellyfin REST client (ports `submate/media_servers/jellyfin.py`).
//!
//! Mirrors the request shapes of the Python `JellyfinClient`: every request
//! carries the `Authorization: MediaBrowser Token=<api_key>` header, and the
//! paths/methods/query params match the Python client exactly so the parity
//! suite can diff recorded requests.

use std::time::Duration;

use serde::Deserialize;

/// Timeout applied to every request, matching the Python client's `timeout=10`.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Errors raised by [`JellyfinClient`].
#[derive(Debug, thiserror::Error)]
pub enum JellyfinError {
    /// Jellyfin is not configured (missing server URL or API key).
    #[error("Jellyfin not configured")]
    NotConfigured,
    /// A method was called before [`JellyfinClient::connect`] succeeded.
    #[error("Not connected to Jellyfin server")]
    NotConnected,
    /// No administrator account was found on the server.
    #[error("No admin user found in Jellyfin")]
    NoAdminUser,
    /// The requested item has no `Path` field.
    #[error("No file path found for item {0}")]
    NoFilePath(String),
    /// The underlying HTTP request failed.
    #[error("Jellyfin API error: {0}")]
    Http(#[from] reqwest::Error),
}

/// Result alias for Jellyfin operations.
pub type Result<T> = std::result::Result<T, JellyfinError>;

/// Webhook notification payload sent by the Jellyfin Webhook plugin.
///
/// Ports `submate/server/handlers/jellyfin/models.py::JellyfinWebhookPayload`.
/// Jellyfin sends PascalCase keys over the wire (`NotificationType`, `ItemId`,
/// …), so those are the canonical `serde` field names. The Python model sets
/// `populate_by_name=True`, which means it also accepts the snake_case form; the
/// matching `serde(alias = …)` keeps the Rust struct bug-for-bug compatible.
///
/// `notification_type` and `item_id` are required (no Python default); the
/// remaining fields are optional and default to `None`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct JellyfinWebhookPayload {
    /// Event kind, e.g. `"ItemAdded"` or `"PlaybackStart"`.
    #[serde(rename = "NotificationType", alias = "notification_type")]
    pub notification_type: String,
    /// Jellyfin item identifier the event refers to.
    #[serde(rename = "ItemId", alias = "item_id")]
    pub item_id: String,
    /// Item type (e.g. `"Movie"`, `"Episode"`), if provided.
    #[serde(rename = "ItemType", alias = "item_type", default)]
    pub item_type: Option<String>,
    /// Human-readable item name, if provided.
    #[serde(rename = "Name", alias = "name", default)]
    pub name: Option<String>,
    /// Originating Jellyfin server identifier, if provided.
    #[serde(rename = "ServerId", alias = "server_id", default)]
    pub server_id: Option<String>,
}

impl JellyfinWebhookPayload {
    /// Whether this is an `ItemAdded` event.
    pub fn is_item_added(&self) -> bool {
        self.notification_type == "ItemAdded"
    }

    /// Whether this is a `PlaybackStart` event.
    pub fn is_playback_start(&self) -> bool {
        self.notification_type == "PlaybackStart"
    }
}

#[derive(Debug, Deserialize)]
struct UserPolicy {
    #[serde(rename = "IsAdministrator", default)]
    is_administrator: bool,
}

#[derive(Debug, Deserialize)]
struct User {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Policy", default)]
    policy: Option<UserPolicy>,
}

#[derive(Debug, Deserialize)]
struct Item {
    #[serde(rename = "Path")]
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VirtualFolder {
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "Id")]
    id: Option<String>,
}

/// Pick the administrator user ID from a `/Users` array.
///
/// Ports `_get_admin_user_id`'s selection loop: returns the `Id` of the first
/// user whose `Policy.IsAdministrator` is true. A missing `Policy` (or one with
/// `IsAdministrator` absent, which `serde` defaults to `false`) is not an admin.
/// Raises [`JellyfinError::NoAdminUser`] when no user qualifies.
fn pick_admin_user_id(users: &[User]) -> Result<String> {
    for user in users {
        if user.policy.as_ref().map(|p| p.is_administrator).unwrap_or(false) {
            return Ok(user.id.clone());
        }
    }
    Err(JellyfinError::NoAdminUser)
}

/// Extract the on-disk path from a `/Users/{admin}/Items/{item}` object.
///
/// Ports `get_file_path`'s truthiness check: a missing or empty `Path` raises
/// [`JellyfinError::NoFilePath`] (Python treats an empty string as "no path").
fn extract_file_path(item: &Item, item_id: &str) -> Result<String> {
    match &item.path {
        Some(path) if !path.is_empty() => Ok(path.clone()),
        _ => Err(JellyfinError::NoFilePath(item_id.to_string())),
    }
}

/// Find a library's ID by name from a `/Library/VirtualFolders` array.
///
/// Ports `refresh_library`'s lookup: returns the matched entry's `Id`. A missing
/// match is **not** an error — the caller logs a warning and skips the refresh —
/// so this returns the outer `None` to mean "no library by that name", and an
/// inner `Some(None)` to mean "matched, but the entry carried no `Id`".
fn match_library_id(libraries: &[VirtualFolder], library_name: &str) -> Option<Option<String>> {
    libraries
        .iter()
        .find(|library| library.name.as_deref() == Some(library_name))
        .map(|library| library.id.clone())
}

/// Client for interacting with a Jellyfin Media Server.
///
/// Ported from `submate/media_servers/jellyfin.py`. The client starts
/// disconnected; [`connect`](Self::connect) validates configuration and probes
/// the server before other methods become usable.
#[derive(Debug, Clone)]
pub struct JellyfinClient {
    http: reqwest::Client,
    server_url: Option<String>,
    api_key: Option<String>,
    libraries: Vec<String>,
    config_server_url: String,
    config_api_key: String,
    admin_user_id: Option<String>,
}

impl JellyfinClient {
    /// Build a client from the Jellyfin configuration values.
    ///
    /// Mirrors `JellyfinClient(config)`: the client is created in a
    /// disconnected state and only records the configured values.
    pub fn new(server_url: impl Into<String>, api_key: impl Into<String>, libraries: Vec<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            server_url: None,
            api_key: None,
            libraries,
            config_server_url: server_url.into(),
            config_api_key: api_key.into(),
            admin_user_id: None,
        }
    }

    /// Whether Jellyfin is configured (server URL and API key are both set).
    pub fn is_configured(&self) -> bool {
        !self.config_server_url.is_empty() && !self.config_api_key.is_empty()
    }

    /// The `Authorization` header value used for every request.
    fn auth_value(&self) -> String {
        let key = self.api_key.as_deref().unwrap_or(&self.config_api_key);
        format!("MediaBrowser Token={key}")
    }

    fn server(&self) -> Result<&str> {
        match (self.server_url.as_deref(), self.api_key.as_deref()) {
            (Some(url), Some(_)) => Ok(url),
            _ => Err(JellyfinError::NotConnected),
        }
    }

    fn get(&self, url: &str) -> reqwest::RequestBuilder {
        self.http
            .get(url)
            .header(reqwest::header::AUTHORIZATION, self.auth_value())
            .timeout(REQUEST_TIMEOUT)
    }

    fn post(&self, url: &str) -> reqwest::RequestBuilder {
        self.http
            .post(url)
            .header(reqwest::header::AUTHORIZATION, self.auth_value())
            .timeout(REQUEST_TIMEOUT)
    }

    /// Connect to the Jellyfin server.
    ///
    /// Validates configuration, then probes `GET /Library/VirtualFolders` to
    /// confirm the credentials work, matching the Python `connect`.
    pub async fn connect(&mut self) -> Result<()> {
        if !self.is_configured() {
            return Err(JellyfinError::NotConfigured);
        }

        tracing::info!(server = %self.config_server_url, "Connecting to Jellyfin server");

        self.server_url = Some(self.config_server_url.clone());
        self.api_key = Some(self.config_api_key.clone());

        let server = self.config_server_url.clone();
        let url = format!("{server}/Library/VirtualFolders");
        self.get(&url).send().await?.error_for_status()?;

        tracing::info!("Connected to Jellyfin successfully");
        Ok(())
    }

    /// Resolve and cache the administrator user ID.
    ///
    /// Ports `_get_admin_user_id`: `GET /Users`, returning the first user whose
    /// policy marks them as an administrator.
    async fn admin_user_id(&mut self) -> Result<String> {
        if let Some(id) = &self.admin_user_id {
            return Ok(id.clone());
        }

        let server = self.server()?.to_string();
        let url = format!("{server}/Users");
        let users: Vec<User> = self.get(&url).send().await?.error_for_status()?.json().await?;

        let admin_id = pick_admin_user_id(&users)?;
        self.admin_user_id = Some(admin_id.clone());
        Ok(admin_id)
    }

    /// Get the on-disk file path for a media item.
    ///
    /// Ports `get_file_path`: resolves the admin user, then
    /// `GET /Users/{adminId}/Items/{itemId}` and returns its `Path`.
    pub async fn get_file_path(&mut self, item_id: &str) -> Result<String> {
        let admin_id = self.admin_user_id().await?;
        let server = self.server()?.to_string();

        let url = format!("{server}/Users/{admin_id}/Items/{item_id}");
        let item: Item = self.get(&url).send().await?.error_for_status()?.json().await?;

        let path = extract_file_path(&item, item_id)?;
        tracing::debug!(item_id, path, "Retrieved file path");
        Ok(path)
    }

    /// Refresh metadata for a specific item.
    ///
    /// Ports `refresh_item`: `POST /Items/{itemId}/Refresh?Recursive=true`.
    pub async fn refresh_item(&self, item_id: &str) -> Result<()> {
        let server = self.server()?.to_string();
        let url = format!("{server}/Items/{item_id}/Refresh");

        self.post(&url)
            .query(&[("Recursive", "true")])
            .send()
            .await?
            .error_for_status()?;

        tracing::debug!(item_id, "Refreshed metadata for item");
        Ok(())
    }

    /// Refresh a specific library by name.
    ///
    /// Ports `refresh_library`: looks the library up in
    /// `GET /Library/VirtualFolders`, then `POST /Items/{libraryId}/Refresh?Recursive=true`.
    /// A missing library logs a warning and is treated as a no-op.
    pub async fn refresh_library(&self, library_name: &str) -> Result<()> {
        let server = self.server()?.to_string();
        tracing::info!(library = library_name, "Refreshing Jellyfin library");

        let url = format!("{server}/Library/VirtualFolders");
        let libraries: Vec<VirtualFolder> =
            self.get(&url).send().await?.error_for_status()?.json().await?;

        match match_library_id(&libraries, library_name) {
            Some(library_id) => {
                if let Some(library_id) = library_id {
                    let refresh_url = format!("{server}/Items/{library_id}/Refresh");
                    self.post(&refresh_url)
                        .query(&[("Recursive", "true")])
                        .send()
                        .await?
                        .error_for_status()?;
                    tracing::info!(library = library_name, "Refreshed library");
                }
                Ok(())
            }
            None => {
                tracing::warn!(library = library_name, "Library not found");
                Ok(())
            }
        }
    }

    /// Refresh all configured libraries.
    ///
    /// Ports `refresh_all_libraries`: iterates the configured library names,
    /// returning those that refreshed without error.
    pub async fn refresh_all_libraries(&self) -> Vec<String> {
        if self.libraries.is_empty() {
            tracing::info!("No Jellyfin libraries configured");
            return Vec::new();
        }

        let mut refreshed = Vec::new();
        for library in &self.libraries {
            match self.refresh_library(library).await {
                Ok(()) => refreshed.push(library.clone()),
                Err(err) => tracing::error!(library, error = %err, "Failed to refresh library"),
            }
        }
        refreshed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_header_uses_mediabrowser_token_form() {
        let client = JellyfinClient::new("http://host:8096", "secret-key", vec![]);
        assert_eq!(client.auth_value(), "MediaBrowser Token=secret-key");
    }

    #[test]
    fn is_configured_requires_url_and_key() {
        assert!(JellyfinClient::new("http://h", "k", vec![]).is_configured());
        assert!(!JellyfinClient::new("", "k", vec![]).is_configured());
        assert!(!JellyfinClient::new("http://h", "", vec![]).is_configured());
    }

    #[test]
    fn webhook_payload_pascalcase() {
        // Jellyfin sends PascalCase keys; optional fields may be absent.
        let payload: JellyfinWebhookPayload = serde_json::from_str(
            r#"{"NotificationType":"ItemAdded","ItemId":"abc","ItemType":"Movie"}"#,
        )
        .unwrap();
        assert_eq!(payload.notification_type, "ItemAdded");
        assert_eq!(payload.item_id, "abc");
        assert_eq!(payload.item_type.as_deref(), Some("Movie"));
        assert_eq!(payload.name, None);
        assert_eq!(payload.server_id, None);
        assert!(payload.is_item_added());
        assert!(!payload.is_playback_start());

        // populate_by_name=True parity: snake_case keys also deserialize.
        let snake: JellyfinWebhookPayload =
            serde_json::from_str(r#"{"notification_type":"PlaybackStart","item_id":"xyz"}"#)
                .unwrap();
        assert_eq!(snake.notification_type, "PlaybackStart");
        assert_eq!(snake.item_id, "xyz");
        assert!(snake.is_playback_start());

        // ItemId is required (no Python default): a payload missing it must fail.
        let missing = serde_json::from_str::<JellyfinWebhookPayload>(
            r#"{"NotificationType":"ItemAdded","ItemType":"Movie"}"#,
        );
        assert!(missing.is_err());
    }
}

/// Unit B — response-parsing parity against the golden Jellyfin bodies.
///
/// These tests drive the private selection helpers (`pick_admin_user_id`,
/// `extract_file_path`, `match_library_id`) with the golden response bodies and
/// assert the derived values match the Python-derived values in `expected.json`.
///
/// `rust/fixtures/` is denylisted for the port, so the goldens are captured
/// separately. Until they land, each golden-dependent test skips with an
/// `eprintln` (same pattern as `submate-media`'s `extract_pcm_sha`) so it arms
/// itself the moment the fixtures appear. The non-golden semantics (no-admin,
/// empty-`Path`, unknown-library) are pinned inline and always run.
#[cfg(test)]
mod parity {
    use super::*;
    use std::path::{Path, PathBuf};

    fn fixtures_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/jellyfin")
    }

    /// Read and parse a golden JSON body, or `None` (with an `eprintln` skip
    /// note) when the fixture has not been captured yet.
    fn golden(name: &str) -> Option<serde_json::Value> {
        let path = fixtures_dir().join(name);
        match std::fs::read_to_string(&path) {
            Ok(text) => Some(
                serde_json::from_str(&text)
                    .unwrap_or_else(|e| panic!("malformed golden {}: {e}", path.display())),
            ),
            Err(_) => {
                eprintln!(
                    "skipping golden assertion: rust/fixtures/jellyfin/{name} not captured yet \
                     (rust/fixtures/ is denylisted — capture first)"
                );
                None
            }
        }
    }

    #[test]
    fn admin_selection_matches_golden() {
        let (Some(users_body), Some(expected)) = (golden("users.json"), golden("expected.json"))
        else {
            return;
        };

        let users: Vec<User> = serde_json::from_value(users_body).expect("users.json shape");
        let admin_id = pick_admin_user_id(&users).expect("golden /Users has an admin");

        let want = expected["admin_user_id"]
            .as_str()
            .expect("expected.json has admin_user_id");
        assert_eq!(admin_id, want, "admin id must equal Python-derived value");
    }

    #[test]
    fn file_path_matches_golden() {
        let (Some(item_body), Some(expected)) = (golden("item.json"), golden("expected.json"))
        else {
            return;
        };

        let item: Item = serde_json::from_value(item_body).expect("item.json shape");
        let path = extract_file_path(&item, "golden-item").expect("golden item has a Path");

        let want = expected["file_path"]
            .as_str()
            .expect("expected.json has file_path");
        assert_eq!(path, want, "file path must equal Python-derived value");
    }

    #[test]
    fn library_lookup_matches_golden() {
        let (Some(folders_body), Some(expected)) =
            (golden("virtual_folders.json"), golden("expected.json"))
        else {
            return;
        };

        let folders: Vec<VirtualFolder> =
            serde_json::from_value(folders_body).expect("virtual_folders.json shape");

        let name = expected["library_name"]
            .as_str()
            .expect("expected.json has library_name");
        let want = expected["library_id"]
            .as_str()
            .expect("expected.json has library_id");

        // Outer Some => matched by Name; inner Some => the entry carried an Id.
        let library_id = match_library_id(&folders, name)
            .expect("golden folders contain the library")
            .expect("matched library has an Id");
        assert_eq!(library_id, want, "library id must equal Python-derived value");
    }

    // ---- Non-golden semantics: always run (no fixture needed). ----

    #[test]
    fn no_admin_user_raises() {
        // Missing Policy => not admin; explicit IsAdministrator=false => not admin.
        let users: Vec<User> = serde_json::from_value(serde_json::json!([
            { "Id": "no-policy" },
            { "Id": "non-admin", "Policy": { "IsAdministrator": false } }
        ]))
        .unwrap();
        assert!(matches!(
            pick_admin_user_id(&users),
            Err(JellyfinError::NoAdminUser)
        ));
    }

    #[test]
    fn first_admin_wins_and_missing_policy_is_not_admin() {
        let users: Vec<User> = serde_json::from_value(serde_json::json!([
            { "Id": "no-policy" },
            { "Id": "admin-a", "Policy": { "IsAdministrator": true } },
            { "Id": "admin-b", "Policy": { "IsAdministrator": true } }
        ]))
        .unwrap();
        assert_eq!(pick_admin_user_id(&users).unwrap(), "admin-a");
    }

    #[test]
    fn empty_or_missing_path_raises_no_file_path() {
        // Empty string is falsy in Python: counts as "no path".
        let empty: Item = serde_json::from_value(serde_json::json!({ "Path": "" })).unwrap();
        assert!(matches!(
            extract_file_path(&empty, "item-x"),
            Err(JellyfinError::NoFilePath(id)) if id == "item-x"
        ));

        // Absent Path field likewise raises.
        let missing: Item = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(matches!(
            extract_file_path(&missing, "item-y"),
            Err(JellyfinError::NoFilePath(id)) if id == "item-y"
        ));
    }

    #[test]
    fn unknown_library_is_not_an_error() {
        // Python logs a warning and returns without refreshing: no match => None,
        // so the caller issues no POST and does not raise.
        let folders: Vec<VirtualFolder> = serde_json::from_value(serde_json::json!([
            { "Name": "Movies", "Id": "library-1" },
            { "Name": "TV Shows", "Id": "library-2" }
        ]))
        .unwrap();
        assert_eq!(match_library_id(&folders, "Nope"), None);
        assert_eq!(
            match_library_id(&folders, "Movies"),
            Some(Some("library-1".to_string()))
        );
    }
}
