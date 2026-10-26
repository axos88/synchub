//! Syncthing's database state of a folder (`/rest/db/...`).

use serde::Deserialize;

/// What `/rest/db/status` returns.
pub type FolderStatusResponse = FolderStatus;

/// How a folder stands here (`/rest/db/status`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderStatus {
    /// idle, scanning, syncing, error, ...
    pub state: String,
    #[serde(default)]
    pub error: String,
    pub local_files: u64,
    #[serde(default)]
    pub need_files: u64,
    #[serde(default)]
    pub need_bytes: u64,
}

/// A folder's ignore patterns (`/rest/db/ignores`).
#[derive(Deserialize)]
pub struct IgnoresResponse {
    /// The problem Syncthing found in them, if any. Not in the API docs, but
    /// in the responses (Syncthing v2.1.7).
    #[serde(default)]
    pub error: Option<String>,
}

/// What `/rest/db/completion` returns.
pub type CompletionResponse = Completion;

/// How far a device has synced a folder, as far as this device knows (`/rest/db/completion`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Completion {
    /// Percent.
    pub completion: f64,
    #[serde(default)]
    pub need_items: u64,
    #[serde(default)]
    pub need_bytes: u64,
}

/// What `/rest/db/file` returns about a file of a folder.
#[derive(Deserialize)]
pub struct FileResponse {
    /// The newest version any device has.
    pub global: File,
}

/// A file as the folder's index has it.
#[derive(Deserialize)]
pub struct File {
    /// It was deleted: what's left in the index is the deletion.
    pub deleted: bool,
}
