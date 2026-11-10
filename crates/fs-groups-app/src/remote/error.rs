//! Why a call to the groups program did not give an answer.
//!
//! Pattern: Adapter — the typed problems of the server (RFC 9457, `groups/*`) and the transport errors become
//! one enum the backend can turn into the frontend's `BackendError`; nothing here is shown as text to a person.

use fs_interfaces::rpc::Problem;

/// What went wrong with a call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunError {
    /// The command is in the description but this program cannot run it yet (the provider rule, ADR 0045): either
    /// the table says so or the server answered `unsupported`. The text names the command.
    Unsupported(String),
    /// There is no such record or command.
    NotFound,
    /// The program refused by its own rules (a cycle, forbidden, expired, a conflict); `kind` is the last part of the
    /// problem type (`cycle`, `forbidden` ...), `detail` is for the log.
    Refused {
        /// Problem code.
        kind: String,
        /// English text for the log.
        detail: String,
    },
    /// A value of the input is not usable (`field` names it): a reference that is no id, a time that is no time.
    Bad {
        /// The input field.
        field: String,
    },
    /// The program cannot be reached or failed; for the log.
    Unavailable(String),
}

impl From<&capnp::Error> for RunError {
    fn from(e: &capnp::Error) -> Self {
        let Some(p) = Problem::from_error(e) else {
            return Self::Unavailable(e.to_string());
        };
        if p.is_unsupported() {
            return Self::Unsupported(p.detail);
        }
        match p.status {
            404 => Self::NotFound,
            400..=499 => Self::Refused {
                kind: p
                    .problem_type
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .to_owned(),
                detail: p.detail,
            },
            _ => Self::Unavailable(p.detail),
        }
    }
}

impl From<capnp::Error> for RunError {
    fn from(e: capnp::Error) -> Self {
        Self::from(&e)
    }
}

impl From<capnp::NotInSchema> for RunError {
    fn from(e: capnp::NotInSchema) -> Self {
        Self::Unavailable(e.to_string())
    }
}

impl From<std::str::Utf8Error> for RunError {
    fn from(e: std::str::Utf8Error) -> Self {
        Self::Unavailable(e.to_string())
    }
}
