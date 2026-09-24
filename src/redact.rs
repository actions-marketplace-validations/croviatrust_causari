//! Secret redaction for text the ledger stores in clear.
//!
//! `re proxy` and the agent hooks keep prompts, completions and shell
//! commands verbatim under `.causari/capture` and in event objects. A prompt
//! that pastes an API key would otherwise be stored as typed. Before any such
//! text is written, credentials in formats that are recognisable with
//! certainty — vendor-prefixed API keys, GitHub and GitLab tokens, Slack and
//! Stripe tokens, AWS access key ids, Google API keys, JWTs, bearer values
//! and PEM private-key blocks — are replaced by `[redacted:<kind>]`. Nothing
//! else is touched: the scan is prefix-anchored, so ordinary words, hashes
//! and identifiers stay as they are. Every writer records how many
//! replacements it made (`redactions`), so a reader knows the text is not
//! the original when it is not.
//!
//! This is a guard against the common accident, not a classifier: a secret
//! with no recognisable shape (a bare password, a home-grown token) passes
//! through. `SECURITY.md` says so.

/// A recognised secret shape: a prefix, the alphabet of the body and the
/// minimum body length. The character before the prefix must not be part
/// of a longer word; the body ends at the first character outside its
/// alphabet.
struct Shape {
    kind: &'static str,
    prefix: &'static str,
    body: fn(u8) -> bool,
    min_body: usize,
    /// The prefix matches regardless of case (`Bearer`, `bearer`).
    case_insensitive: bool,
}

fn alnum(c: u8) -> bool {
    c.is_ascii_alphanumeric()
}

fn alnum_dash_us(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b'_'
}

fn alnum_us(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn upper_digit(c: u8) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit()
}

fn bearer_body(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'~' | b'+' | b'/' | b'=' | b'-')
}

const SHAPES: &[Shape] = &[
    // OpenAI (`sk-`, `sk-proj-`), Anthropic (`sk-ant-`), and every vendor
    // that adopted the prefix.
    Shape {
        kind: "api-key",
        prefix: "sk-",
        body: alnum_dash_us,
        min_body: 20,
        case_insensitive: false,
    },
    Shape {
        kind: "stripe-key",
        prefix: "sk_live_",
        body: alnum,
        min_body: 16,
        case_insensitive: false,
    },
    Shape {
        kind: "stripe-key",
        prefix: "sk_test_",
        body: alnum,
        min_body: 16,
        case_insensitive: false,
    },
    Shape {
        kind: "stripe-key",
        prefix: "rk_live_",
        body: alnum,
        min_body: 16,
        case_insensitive: false,
    },
    Shape {
        kind: "github-token",
        prefix: "github_pat_",
        body: alnum_us,
        min_body: 20,
        case_insensitive: false,
    },
    Shape {
        kind: "github-token",
        prefix: "ghp_",
        body: alnum,
        min_body: 30,
        case_insensitive: false,
    },
    Shape {
        kind: "github-token",
        prefix: "gho_",
        body: alnum,
        min_body: 30,
        case_insensitive: false,
    },
    Shape {
        kind: "github-token",
        prefix: "ghu_",
        body: alnum,
        min_body: 30,
        case_insensitive: false,
    },
    Shape {
        kind: "github-token",
        prefix: "ghs_",
        body: alnum,
        min_body: 30,
        case_insensitive: false,
    },
    Shape {
        kind: "github-token",
        prefix: "ghr_",
        body: alnum,
        min_body: 30,
        case_insensitive: false,
    },
    Shape {
        kind: "gitlab-token",
        prefix: "glpat-",
        body: alnum_dash_us,
        min_body: 20,
        case_insensitive: false,
    },
    Shape {
        kind: "slack-token",
        prefix: "xoxb-",
        body: alnum_dash_us,
        min_body: 10,
        case_insensitive: false,
    },
    Shape {
        kind: "slack-token",
        prefix: "xoxp-",
        body: alnum_dash_us,
        min_body: 10,
        case_insensitive: false,
    },
    Shape {
        kind: "slack-token",
        prefix: "xoxa-",
        body: alnum_dash_us,
        min_body: 10,
        case_insensitive: false,
    },
    Shape {
        kind: "slack-token",
        prefix: "xoxr-",
        body: alnum_dash_us,
        min_body: 10,
        case_insensitive: false,
    },
    Shape {
        kind: "slack-token",
        prefix: "xoxs-",
        body: alnum_dash_us,
        min_body: 10,
        case_insensitive: false,
    },
    Shape {
        kind: "huggingface-token",
        prefix: "hf_",
        body: alnum,
        min_body: 30,
        case_insensitive: false,
    },
    Shape {
        kind: "npm-token",
        prefix: "npm_",
        body: alnum,
        min_body: 30,
        case_insensitive: false,
    },
    Shape {
        kind: "pypi-token",
        prefix: "pypi-",
        body: alnum_dash_us,
        min_body: 50,
        case_insensitive: false,
    },
    Shape {
        kind: "aws-access-key-id",
        prefix: "AKIA",
        body: upper_digit,
        min_body: 16,
        case_insensitive: false,
    },
    Shape {
        kind: "google-api-key",
        prefix: "AIza",
        body: alnum_dash_us,
        min_body: 35,
        case_insensitive: false,
    },
    Shape {
        kind: "bearer-token",
        prefix: "Bearer ",
        body: bearer_body,
        min_body: 20,
        case_insensitive: true,
    },
];

const PEM_BEGIN: &str = "-----BEGIN ";
const PEM_KEY: &str = "PRIVATE KEY-----";
const PEM_END: &str = "-----END ";

/// The sentence every capturing command prints once at start: what is
/// stored, where, and what the redaction does and does not catch.
pub const STORAGE_NOTICE: &str = "Prompts, completions and commands are stored in clear under .causari/ (gitignored, this machine only). \
Credentials in recognised formats are replaced by [redacted:<kind>] before writing; anything else pasted is kept as typed. \
Treat .causari/ as sensitive — see SECURITY.md.";

/// The text with every recognised secret replaced, and how many were.
/// Returns the input unchanged (and 0) when nothing matched, so callers can
/// keep the original bytes on the common path.
pub fn redact(text: &str) -> (String, u32) {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut count = 0u32;
    let mut i = 0;
    while i < bytes.len() {
        if let Some((end, kind)) = match_at(text, i) {
            out.push_str("[redacted:");
            out.push_str(kind);
            out.push(']');
            count += 1;
            i = end;
            continue;
        }
        // Advance by one UTF-8 character, not one byte.
        let ch_len = utf8_len(bytes[i]);
        out.push_str(&text[i..i + ch_len]);
        i += ch_len;
    }
    if count == 0 {
        return (text.to_string(), 0);
    }
    (out, count)
}

fn utf8_len(first: u8) -> usize {
    match first {
        b if b < 0x80 => 1,
        b if b >> 5 == 0b110 => 2,
        b if b >> 4 == 0b1110 => 3,
        _ => 4,
    }
}

/// The secret starting exactly at byte `i`, if any: its end offset and kind.
fn match_at(text: &str, i: usize) -> Option<(usize, &'static str)> {
    let bytes = text.as_bytes();
    let boundary = i == 0 || !alnum_us(bytes[i - 1]);
    if !boundary {
        return None;
    }
    let rest = &text[i..];
    if rest.starts_with(PEM_BEGIN) {
        return pem_block(text, i);
    }
    if let Some(end) = jwt(rest) {
        return Some((i + end, "jwt"));
    }
    for s in SHAPES {
        let matched = if s.case_insensitive {
            rest.as_bytes()
                .get(..s.prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(s.prefix.as_bytes()))
        } else {
            rest.starts_with(s.prefix)
        };
        if !matched {
            continue;
        }
        let body = &rest.as_bytes()[s.prefix.len()..];
        let n = body.iter().take_while(|&&c| (s.body)(c)).count();
        if n >= s.min_body {
            return Some((i + s.prefix.len() + n, s.kind));
        }
    }
    None
}

/// `-----BEGIN … PRIVATE KEY-----` through the matching `-----END … PRIVATE
/// KEY-----`, or to the end of the text when the block is cut short. Public
/// keys and certificates are left alone.
fn pem_block(text: &str, i: usize) -> Option<(usize, &'static str)> {
    let rest = &text[i..];
    let close = rest[PEM_BEGIN.len()..].find("-----")?;
    let header = &rest[..PEM_BEGIN.len() + close + 5];
    if !header.ends_with(PEM_KEY) {
        return None;
    }
    let label = &header[PEM_BEGIN.len()..header.len() - 5];
    let footer = format!("{PEM_END}{label}-----");
    let end = match rest.find(&footer) {
        Some(pos) => pos + footer.len(),
        None => rest.len(),
    };
    Some((i + end, "private-key"))
}

/// Three base64url segments, the first two starting with `eyJ` (`{"` in
/// base64), the signature at least 10 characters.
fn jwt(rest: &str) -> Option<usize> {
    let b = rest.as_bytes();
    let seg =
        |from: usize| -> usize { b[from..].iter().take_while(|&&c| alnum_dash_us(c)).count() };
    if !rest.starts_with("eyJ") {
        return None;
    }
    let a = seg(0);
    if a < 10 || b.get(a) != Some(&b'.') || !rest[a + 1..].starts_with("eyJ") {
        return None;
    }
    let p = seg(a + 1);
    if p < 10 || b.get(a + 1 + p) != Some(&b'.') {
        return None;
    }
    let s = seg(a + 2 + p);
    if s < 10 {
        return None;
    }
    Some(a + 2 + p + s)
}

/// `Some(text)` redacted in place for an optional field; the count is added
/// to `total`.
pub fn redact_opt(text: &mut Option<String>, total: &mut u32) {
    if let Some(t) = text {
        let (r, n) = redact(t);
        if n > 0 {
            *t = r;
            *total += n;
        }
    }
}

/// Redact a required field; the count is added to `total`.
pub fn redact_str(text: &mut String, total: &mut u32) {
    let (r, n) = redact(text);
    if n > 0 {
        *text = r;
        *total += n;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> (String, u32) {
        redact(text)
    }

    /// Test secrets are assembled at run time so the source holds no string
    /// a secret scanner (GitHub push protection included) would flag.
    fn fake(prefix: &str, body: &str) -> String {
        format!("{prefix}{body}")
    }

    const BODY: &str = "abcdefghijklmnopqrstuvwxyz0123456789";
    const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef0123456789";

    #[test]
    fn vendor_keys_are_replaced_and_counted() {
        let cases = [
            (
                format!("use {} now", fake("sk-proj-", BODY)),
                "use [redacted:api-key] now",
                "openai",
            ),
            (
                format!("key={}", fake("sk-ant-api03-", UPPER)),
                "key=[redacted:api-key]",
                "anthropic",
            ),
            (fake("ghp_", UPPER), "[redacted:github-token]", "ghp"),
            (
                fake(
                    "github_pat_",
                    "11ABCDEFG0123456789_abcdefghijklmnopqrstuvwxyz",
                ),
                "[redacted:github-token]",
                "pat",
            ),
            (fake("glpat-", BODY), "[redacted:gitlab-token]", "gitlab"),
            (
                fake("xoxb-", "123456789012-abcdefghijkl"),
                "[redacted:slack-token]",
                "slack",
            ),
            (
                fake("AKIA", "IOSFODNN7EXAMPLE"),
                "[redacted:aws-access-key-id]",
                "aws",
            ),
            (
                fake("AIza", "SyA-abcdefghijklmnopqrstuvwxyz0123456789"),
                "[redacted:google-api-key]",
                "google",
            ),
            (fake("hf_", UPPER), "[redacted:huggingface-token]", "hf"),
            (fake("npm_", UPPER), "[redacted:npm-token]", "npm"),
            (
                fake("sk_live_", "abcdefghijklmnopqrstuvwx"),
                "[redacted:stripe-key]",
                "stripe",
            ),
            (
                format!(
                    "Authorization: {}",
                    fake("Bearer ", "abcdefghijklmnopqrstuvwxyz.0123456789")
                ),
                "Authorization: [redacted:bearer-token]",
                "bearer",
            ),
            (
                format!("authorization: {}", fake("bearer ", BODY)),
                "authorization: [redacted:bearer-token]",
                "bearer lower",
            ),
            (
                fake(
                    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.",
                    "eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
                ),
                "[redacted:jwt]",
                "jwt",
            ),
        ];
        for (input, expected, name) in cases {
            let (out, n) = one(&input);
            assert_eq!(out, expected, "{name}");
            assert_eq!(n, 1, "{name} count");
        }
    }

    #[test]
    fn ordinary_text_is_untouched() {
        let cases = [
            "the task-list is ready".to_string(),
            "sk-short".to_string(),
            "ghp_tooshort".to_string(),
            "commit 1500353f09975b832793297a3c1f0e9b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f".to_string(),
            "Bearer of bad news".to_string(),
            "AKIA is a prefix, AKIAIOSFODNN7EXAMPL is fifteen".to_string(),
            format!("ri{} is inside a word", fake("sk-proj-", BODY)),
            "eyJhbGciOiJIUzI1NiJ9 alone is not a jwt".to_string(),
            "-----BEGIN PUBLIC KEY-----\nMFkw\n-----END PUBLIC KEY-----".to_string(),
            "naïve — unicode stays: 日本語 ✓".to_string(),
        ];
        for input in cases {
            let (out, n) = one(&input);
            assert_eq!(n, 0, "{input:?}");
            assert_eq!(out, input);
        }
    }

    #[test]
    fn pem_private_key_block_is_one_redaction() {
        let text = "here:\n-----BEGIN RSA PRIVATE KEY-----\nMIIE\nAAAA\n-----END RSA PRIVATE KEY-----\nthen";
        let (out, n) = one(text);
        assert_eq!(n, 1);
        assert_eq!(out, "here:\n[redacted:private-key]\nthen");
        let openssh =
            "-----BEGIN OPENSSH PRIVATE KEY-----\nb3Blbn\n-----END OPENSSH PRIVATE KEY-----";
        assert_eq!(one(openssh), ("[redacted:private-key]".into(), 1));
        // cut short: redacted to the end rather than left in clear
        let cut = "x -----BEGIN EC PRIVATE KEY-----\nMHcCAQEE";
        assert_eq!(one(cut), ("x [redacted:private-key]".into(), 1));
    }

    #[test]
    fn several_secrets_in_one_text_are_all_counted() {
        let text = format!("a {} b {} c", fake("sk-", BODY), fake("ghp_", UPPER));
        let (out, n) = one(&text);
        assert_eq!(n, 2);
        assert_eq!(out, "a [redacted:api-key] b [redacted:github-token] c");
    }

    #[test]
    fn helpers_touch_only_what_matched() {
        let mut total = 0;
        let mut none = Some("plain".to_string());
        redact_opt(&mut none, &mut total);
        assert_eq!(none.as_deref(), Some("plain"));
        let mut hit = Some(fake("sk-", BODY));
        redact_opt(&mut hit, &mut total);
        assert_eq!(hit.as_deref(), Some("[redacted:api-key]"));
        let mut s = format!("token {}", fake("xoxb-", "1234567890-abc"));
        redact_str(&mut s, &mut total);
        assert_eq!(s, "token [redacted:slack-token]");
        assert_eq!(total, 2);
    }
}
