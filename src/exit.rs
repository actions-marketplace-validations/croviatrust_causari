//! Process exit statuses chosen by a command.
//!
//! `main` exits 1 on any error. A command that wants a different status —
//! 2 for "the request itself is unusable" (a shallow clone, a retired
//! command) as opposed to 1 for "the thing checked is wrong" — wraps its
//! error in [`ExitWith`]; the message chain is preserved untouched.

use std::fmt;

#[derive(Debug)]
pub struct ExitWith {
    pub code: i32,
    inner: anyhow::Error,
}

impl fmt::Display for ExitWith {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl std::error::Error for ExitWith {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.chain().nth(1)
    }
}

/// Wrap `err` so the process exits with `code` when it reaches `main`.
pub fn exit_with(code: i32, err: anyhow::Error) -> anyhow::Error {
    anyhow::Error::new(ExitWith { code, inner: err })
}

/// The status `err` asks for; 1 when it does not ask.
pub fn code_of(err: &anyhow::Error) -> i32 {
    err.downcast_ref::<ExitWith>().map(|e| e.code).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn exit_code_travels_through_context_and_keeps_the_message() {
        let err = exit_with(2, anyhow!("root").context("outer"));
        assert_eq!(code_of(&err), 2);
        assert_eq!(err.to_string(), "outer");
        assert_eq!(format!("{err:#}"), "outer: root");
        let wrapped = err.context("later");
        assert_eq!(code_of(&wrapped), 2);
        assert_eq!(code_of(&anyhow!("plain")), 1);
    }
}
