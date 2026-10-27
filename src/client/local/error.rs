//! What a Syncthing API call fails with, when callers tell it apart.

use derive_more::{Display, Error};
use reqwest::StatusCode;

/// Syncthing refusing a request: what it answered.
#[derive(Debug, Display, Error)]
#[display("{who} answered {status}: {body}")]
pub struct Refused {
    pub(super) who: &'static str,
    pub(super) status: StatusCode,
    pub(super) body: String,
}

/// The object an operation by ID was for isn't there: e.g. the folder
/// `abcde-12345`, on the hub.
#[derive(Debug, Display, Error)]
#[display("{who} has no {what}")]
pub struct NotFound {
    pub(super) who: &'static str,
    /// What it is and its ID, e.g. "folder abcde-12345".
    pub(super) what: String,
}

/// Whether `err` is Syncthing answering that the folder restarted while the
/// request waited for it (verified, v2.1.7: a scan asked for during a
/// restart gets `500 context canceled`).
pub(super) fn is_cancelled(err: &anyhow::Error) -> bool {
    err.downcast_ref::<Refused>().is_some_and(|refused| {
        refused.status == StatusCode::INTERNAL_SERVER_ERROR && refused.body == "context canceled"
    })
}

/// Whether `err` is Syncthing answering 404.
pub(super) fn is_404(err: &anyhow::Error) -> bool {
    err.downcast_ref::<Refused>()
        .is_some_and(|refused| refused.status == StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_say_who_and_what() {
        let refused = Refused {
            who: "the hub",
            status: StatusCode::CONFLICT,
            body: "folder exists".to_string(),
        };
        assert_eq!(
            refused.to_string(),
            "the hub answered 409 Conflict: folder exists"
        );
        let not_found = NotFound {
            who: "the hub",
            what: "folder abcde-12345".to_string(),
        };
        assert_eq!(not_found.to_string(), "the hub has no folder abcde-12345");
        let cancelled = Refused {
            who: "the hub",
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: "context canceled".to_string(),
        };
        assert!(is_cancelled(&anyhow::Error::new(cancelled)));
        assert!(!is_cancelled(&anyhow::Error::new(Refused {
            status: StatusCode::CONFLICT,
            body: "folder exists".to_string(),
            who: "the hub",
        })));
        assert!(is_404(&anyhow::Error::new(Refused {
            status: StatusCode::NOT_FOUND,
            ..refused
        })));
    }
}
