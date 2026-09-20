//! The audit result as a Crovia Seal.
//!
//! `re audit --seal` issues an ordinary `crovia.seal.v1` (see `seal.rs`) whose
//! subject is one audit:
//!
//! * `subject.output` is the audit JSON — the exact bytes `re audit --json`
//!   prints. Floats live there, never in the seal: the seal commits to them
//!   by hash and length only, as the spec commits to any output.
//! * `subject.input` is the audited git state, CSC-1 canonical: commit hash,
//!   method version and the options that shape the measurement
//!   ([`INPUT_TYPE`]). The same bytes are what a verifier recomputes.
//! * `generator` is this program: `id = "causari"`, `version` = crate
//!   version, `params` (all strings) repeat the binding so it is readable
//!   from the seal alone: `subject_type`, `method`, `commit`, `repo`,
//!   `coverage.shallow`.
//! * `modality = "text"` — the spec's vocabulary has no value for structured
//!   data; a JSON document is text.
//!
//! The spec has no field for "what kind of output this is", so the type of
//! the subject is stated in `generator.params.subject_type`, which is inside
//! the signed payload. Nothing outside the spec's field set is added to the
//! seal itself; the bundle around it (this file's format,
//! [`BUNDLE_KIND`]) carries the two subjects so the hashes can be recomputed
//! without the repository.
//!
//! What a valid audit seal proves: this issuer key signed this exact audit
//! JSON for this commit, produced with this method version, and the numbers
//! were not altered since. What it does not prove: that the numbers are
//! true. Anyone can rerun `re audit` on the commit and compare.

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Value};
use std::path::Path;

use crate::seal::{self, SealGenerator, SealIssuer, SealSubject};

/// Top-level `bundle` value of an `audit.seal.json`.
pub const BUNDLE_KIND: &str = "causari.audit-seal.v1";
/// `type` of the canonical input object.
pub const INPUT_TYPE: &str = "causari.audit.input.v1";
/// `generator.params.subject_type` of an audit seal.
pub const SUBJECT_TYPE: &str = "causari.audit.v1";
/// `generator.id` of an audit seal.
pub const GENERATOR_ID: &str = "causari";

/// What an audit seal binds the audit JSON to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditBinding {
    /// 40-hex commit the audit measured (HEAD of the audited tree).
    pub commit: String,
    /// Method version that produced the numbers (`coverage.method`).
    pub method: String,
    /// `--allow-shallow` was passed.
    pub allow_shallow: bool,
    /// The repository was a shallow clone (`coverage.shallow`).
    pub shallow: bool,
    /// Origin URL without credentials, or `sha256:` of the local path.
    pub repo: String,
}

impl AuditBinding {
    /// The canonical input object: the audited git state. CSC-1 serialisable
    /// by construction (strings and booleans only).
    pub fn input_value(&self) -> Value {
        serde_json::json!({
            "type": INPUT_TYPE,
            "commit": self.commit,
            "method": self.method,
            "options": { "allow_shallow": self.allow_shallow }
        })
    }

    fn params(&self) -> Vec<(String, String)> {
        vec![
            ("subject_type".into(), SUBJECT_TYPE.into()),
            ("method".into(), self.method.clone()),
            ("commit".into(), self.commit.clone()),
            ("repo".into(), self.repo.clone()),
            ("coverage.shallow".into(), self.shallow.to_string()),
        ]
    }
}

/// A remote URL fit for a signed, shareable receipt: user info stripped
/// (`https://x-access-token:…@github.com/o/r` is what CI checkouts carry).
pub fn sanitize_repo_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        // scp-like `git@github.com:o/r.git` names a user, not a secret.
        return url.to_string();
    };
    match rest.split_once('/') {
        Some((authority, path)) => {
            let host = authority.rsplit('@').next().unwrap_or(authority);
            format!("{scheme}://{host}/{path}")
        }
        None => {
            let host = rest.rsplit('@').next().unwrap_or(rest);
            format!("{scheme}://{host}")
        }
    }
}

/// The `repo` parameter: the origin URL when there is one, else a digest of
/// the local path so the seal names the repository without leaking it.
pub fn repo_label(dir: &Path) -> String {
    if let Some(url) = crate::audit::origin_url(dir) {
        return sanitize_repo_url(&url);
    }
    let canonical = dir
        .canonicalize()
        .unwrap_or_else(|_| dir.to_path_buf())
        .to_string_lossy()
        .into_owned();
    format!("sha256:{}", seal::sha256_hex(canonical.as_bytes()))
}

/// Issue one seal over `audit_json` (the exact bytes printed by
/// `re audit --json`) and wrap it with both subjects in a bundle.
pub fn issue(issuer: &mut SealIssuer, audit_json: &[u8], binding: &AuditBinding) -> Result<Value> {
    let input = binding.input_value();
    let input_bytes = seal::csc1_serialize(&input)?;
    let version = env!("CARGO_PKG_VERSION");
    let sealed = issuer.emit(
        SealSubject {
            input: &input_bytes,
            output: audit_json,
            modality: "text",
        },
        SealGenerator {
            id: GENERATOR_ID,
            version: Some(version),
            params: binding.params(),
        },
    )?;
    let audit_text = std::str::from_utf8(audit_json).context("audit JSON is not UTF-8")?;
    Ok(serde_json::json!({
        "bundle": BUNDLE_KIND,
        "seal": sealed,
        "subject": {
            "input": input,
            "audit_json": audit_text
        }
    }))
}

/// True when `value` is an audit seal bundle (as opposed to a bare seal).
pub fn is_bundle(value: &Value) -> bool {
    value.get("bundle").and_then(Value::as_str) == Some(BUNDLE_KIND)
}

/// Everything a verified bundle states, read from the signed seal.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerifiedAudit {
    pub seal_id: String,
    pub issuer_id: String,
    pub pubkey_hex: String,
    pub sequence: u64,
    pub prev_seal_hash: Option<String>,
    pub emitted_at: String,
    pub generator_version: Option<String>,
    pub commit: String,
    pub method: String,
    pub repo: String,
    pub shallow: bool,
    /// The audit JSON, parsed, for display. The bytes are what was sealed.
    pub audit: Value,
}

fn field<'a>(v: &'a Value, path: &[&str]) -> Result<&'a Value> {
    let mut cur = v;
    for key in path {
        cur = cur
            .get(key)
            .ok_or_else(|| anyhow!("bundle: missing {}", path.join(".")))?;
    }
    Ok(cur)
}

fn field_str<'a>(v: &'a Value, path: &[&str]) -> Result<&'a str> {
    field(v, path)?
        .as_str()
        .ok_or_else(|| anyhow!("bundle: {} must be a string", path.join(".")))
}

fn only_keys(obj: &Map<String, Value>, allowed: &[&str], what: &str) -> Result<()> {
    if let Some(k) = obj.keys().find(|k| !allowed.contains(&k.as_str())) {
        bail!("bundle: unknown field {what}.{k} (fail-closed)");
    }
    Ok(())
}

/// Verify an audit seal bundle offline: the seal itself (structure and
/// Ed25519 signature), both subject hashes recomputed from the bundle, and
/// the binding between input, generator params and the audit JSON. Fail
/// closed: any unknown field, mismatch or missing binding is an error.
pub fn verify_bundle(bundle: &Value) -> Result<VerifiedAudit> {
    let obj = bundle
        .as_object()
        .ok_or_else(|| anyhow!("bundle must be a JSON object"))?;
    only_keys(obj, &["bundle", "seal", "subject"], "")?;
    if !is_bundle(bundle) {
        bail!("not an audit seal bundle (expected \"bundle\": \"{BUNDLE_KIND}\")");
    }
    let sealed = field(bundle, &["seal"])?;
    let subject = field(bundle, &["subject"])?
        .as_object()
        .ok_or_else(|| anyhow!("bundle: subject must be an object"))?;
    only_keys(subject, &["input", "audit_json"], "subject")?;

    // 1. The seal on its own terms.
    seal::verify_seal(sealed)?;

    // 2. Output: the audit JSON bytes.
    let audit_text = field_str(bundle, &["subject", "audit_json"])?;
    let audit_bytes = audit_text.as_bytes();
    let want_out = field_str(sealed, &["subject", "output_hash"])?;
    let got_out = format!("sha256:{}", seal::sha256_hex(audit_bytes));
    if want_out != got_out {
        bail!("audit JSON does not match subject.output_hash: the numbers were altered");
    }
    if field(sealed, &["subject", "output_len"])?.as_u64() != Some(audit_bytes.len() as u64) {
        bail!("audit JSON length does not match subject.output_len");
    }
    let audit = seal::parse_json_strict(audit_text).context("audit JSON does not parse")?;
    if !audit.is_object() {
        bail!("audit JSON is not an object");
    }

    // 3. Input: the audited git state, CSC-1 canonical.
    let input = field(bundle, &["subject", "input"])?;
    let input_obj = input
        .as_object()
        .ok_or_else(|| anyhow!("bundle: subject.input must be an object"))?;
    only_keys(
        input_obj,
        &["type", "commit", "method", "options"],
        "subject.input",
    )?;
    if field_str(input, &["type"])? != INPUT_TYPE {
        bail!("subject.input.type is not {INPUT_TYPE}");
    }
    let input_bytes =
        seal::csc1_serialize(input).context("subject.input is not CSC-1 canonical")?;
    let want_in = field_str(sealed, &["subject", "input_hash"])?;
    let got_in = format!("sha256:{}", seal::sha256_hex(&input_bytes));
    if want_in != got_in {
        bail!(
            "subject.input does not match subject.input_hash: the commit or options were altered"
        );
    }
    if field(sealed, &["subject", "input_len"])?.as_u64() != Some(input_bytes.len() as u64) {
        bail!("subject.input length does not match subject.input_len");
    }
    if field_str(sealed, &["subject", "modality"])? != "text" {
        bail!("an audit seal has modality \"text\"");
    }

    // 4. Binding: input, generator params and the audit agree.
    if field_str(sealed, &["generator", "id"])? != GENERATOR_ID {
        bail!("generator.id is not {GENERATOR_ID}");
    }
    let params = field(sealed, &["generator", "params"])?;
    if field_str(params, &["subject_type"])? != SUBJECT_TYPE {
        bail!("generator.params.subject_type is not {SUBJECT_TYPE}");
    }
    let commit = field_str(input, &["commit"])?;
    if commit.len() != 40 || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("subject.input.commit is not a 40-hex commit hash");
    }
    if field_str(params, &["commit"])? != commit {
        bail!("generator.params.commit differs from subject.input.commit");
    }
    let method = field_str(input, &["method"])?;
    if field_str(params, &["method"])? != method {
        bail!("generator.params.method differs from subject.input.method");
    }
    for path in [&["method"][..], &["coverage", "method"][..]] {
        if field_str(&audit, path)? != method {
            bail!(
                "audit JSON {} differs from the sealed method {method}",
                path.join(".")
            );
        }
    }
    let shallow = field(&audit, &["coverage", "shallow"])?
        .as_bool()
        .ok_or_else(|| anyhow!("audit JSON coverage.shallow must be a boolean"))?;
    if field_str(params, &["coverage.shallow"])? != shallow.to_string() {
        bail!("generator.params.coverage.shallow differs from the audit JSON");
    }
    let allow_shallow = field(input, &["options", "allow_shallow"])?
        .as_bool()
        .ok_or_else(|| anyhow!("subject.input.options.allow_shallow must be a boolean"))?;
    if shallow && !allow_shallow {
        bail!("audit of a shallow clone without --allow-shallow cannot have been issued");
    }

    Ok(VerifiedAudit {
        seal_id: field_str(sealed, &["seal_id"])?.to_string(),
        issuer_id: field_str(sealed, &["issuer", "id"])?.to_string(),
        pubkey_hex: field_str(sealed, &["issuer", "pubkey", "key_hex"])?.to_string(),
        sequence: field(sealed, &["chain", "sequence"])?
            .as_u64()
            .ok_or_else(|| anyhow!("chain.sequence must be an integer"))?,
        prev_seal_hash: field(sealed, &["chain", "prev_seal_hash"])?
            .as_str()
            .map(String::from),
        emitted_at: field_str(sealed, &["timestamp", "emitted_at"])?.to_string(),
        generator_version: field(sealed, &["generator", "version"])?
            .as_str()
            .map(String::from),
        commit: commit.to_string(),
        method: method.to_string(),
        repo: field_str(params, &["repo"])?.to_string(),
        shallow,
        audit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::Repo;

    const AUDIT: &str = "{\n  \"coverage\": {\n    \"method\": \"v2\",\n    \"shallow\": false\n  },\n  \"method\": \"v2\",\n  \"verified\": {\n    \"commits\": 1,\n    \"introduced\": 3,\n    \"survival_rate\": 0.6666666666666666,\n    \"surviving\": 2\n  }\n}\n";

    fn binding() -> AuditBinding {
        AuditBinding {
            commit: "a".repeat(40),
            method: "v2".into(),
            allow_shallow: false,
            shallow: false,
            repo: "https://github.com/croviatrust/causari".into(),
        }
    }

    fn issued() -> (tempfile::TempDir, Value) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        let mut issuer = SealIssuer::load_or_create(&repo, None).unwrap();
        let bundle = issue(&mut issuer, AUDIT.as_bytes(), &binding()).unwrap();
        (tmp, bundle)
    }

    #[test]
    fn issue_then_verify_round_trip() {
        let (_tmp, bundle) = issued();
        assert!(is_bundle(&bundle));
        let v = verify_bundle(&bundle).unwrap();
        assert_eq!(v.commit, "a".repeat(40));
        assert_eq!(v.method, "v2");
        assert_eq!(v.sequence, 0);
        assert!(!v.shallow);
        assert_eq!(v.repo, "https://github.com/croviatrust/causari");
        assert_eq!(
            v.generator_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(v.audit["verified"]["surviving"], 2);
        // The seal never carries a float; the audit JSON keeps its bytes.
        let seal_text = serde_json::to_string(&bundle["seal"]).unwrap();
        assert!(!seal_text.contains("0.666"));
        assert_eq!(bundle["subject"]["audit_json"].as_str().unwrap(), AUDIT);
        assert_eq!(bundle["seal"]["subject"]["modality"], "text");
        assert_eq!(
            bundle["seal"]["generator"]["params"]["subject_type"],
            SUBJECT_TYPE
        );
        assert_eq!(
            bundle["seal"]["generator"]["params"]["coverage.shallow"],
            "false"
        );

        // The plain seal verifier accepts the inner seal as-is; a text file
        // of the bundle round-trips through strict parsing.
        seal::verify_seal(&bundle["seal"]).unwrap();
        let text = serde_json::to_string_pretty(&bundle).unwrap();
        verify_bundle(&seal::parse_json_strict(&text).unwrap()).unwrap();
    }

    #[test]
    fn tampered_audit_json_is_rejected() {
        let (_tmp, bundle) = issued();
        let mut b = bundle.clone();
        b["subject"]["audit_json"] =
            Value::String(AUDIT.replace("\"surviving\": 2", "\"surviving\": 3"));
        let err = verify_bundle(&b).unwrap_err().to_string();
        assert!(err.contains("numbers were altered"), "{err}");

        // Fixing the hash to match the new bytes breaks the signature.
        let mut b = bundle.clone();
        let altered = AUDIT.replace("\"surviving\": 2", "\"surviving\": 3");
        b["seal"]["subject"]["output_hash"] =
            Value::String(format!("sha256:{}", seal::sha256_hex(altered.as_bytes())));
        b["subject"]["audit_json"] = Value::String(altered);
        let err = verify_bundle(&b).unwrap_err().to_string();
        assert!(err.contains("signature"), "{err}");
    }

    #[test]
    fn wrong_commit_is_rejected() {
        let (_tmp, bundle) = issued();
        // Commit swapped in the input only: the input hash no longer matches.
        let mut b = bundle.clone();
        b["subject"]["input"]["commit"] = Value::String("b".repeat(40));
        let err = verify_bundle(&b).unwrap_err().to_string();
        assert!(err.contains("input_hash"), "{err}");

        // Commit swapped in the signed params only: the signature fails.
        let mut b = bundle.clone();
        b["seal"]["generator"]["params"]["commit"] = Value::String("b".repeat(40));
        let err = verify_bundle(&b).unwrap_err().to_string();
        assert!(err.contains("signature"), "{err}");

        // Method swapped in the audit JSON: output hash fails; in the input:
        // input hash fails.
        let mut b = bundle.clone();
        b["subject"]["input"]["method"] = Value::String("v1".into());
        assert!(verify_bundle(&b).is_err());
    }

    #[test]
    fn bundle_fails_closed_on_unknown_fields_and_wrong_kinds() {
        let (_tmp, bundle) = issued();
        let mut b = bundle.clone();
        b["note"] = Value::String("SOC2 certified".into());
        let err = verify_bundle(&b).unwrap_err().to_string();
        assert!(err.contains("unknown field"), "{err}");

        let mut b = bundle.clone();
        b["subject"]["claim"] = Value::String("x".into());
        assert!(verify_bundle(&b).is_err());

        let mut b = bundle.clone();
        b["bundle"] = Value::String("something.else".into());
        assert!(verify_bundle(&b).is_err());

        // A bare exchange seal is not an audit bundle.
        assert!(!is_bundle(&bundle["seal"]));
        assert!(verify_bundle(&bundle["seal"]).is_err());
    }

    #[test]
    fn shallow_binding_is_carried_and_checked() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        let mut issuer = SealIssuer::load_or_create(&repo, None).unwrap();
        let audit = AUDIT.replace("\"shallow\": false", "\"shallow\": true");
        let mut binding = binding();
        binding.shallow = true;
        binding.allow_shallow = true;
        let bundle = issue(&mut issuer, audit.as_bytes(), &binding).unwrap();
        assert_eq!(
            bundle["seal"]["generator"]["params"]["coverage.shallow"],
            "true"
        );
        let v = verify_bundle(&bundle).unwrap();
        assert!(v.shallow);

        // A seal whose params say "not shallow" over a shallow audit is a
        // contradiction, whichever side was edited.
        let mut binding = self::binding();
        binding.shallow = false;
        binding.allow_shallow = true;
        let bad = issue(&mut issuer, audit.as_bytes(), &binding).unwrap();
        let err = verify_bundle(&bad).unwrap_err().to_string();
        assert!(err.contains("coverage.shallow"), "{err}");
    }

    #[test]
    fn audit_seals_share_the_chain_with_exchange_seals() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        let mut issuer = SealIssuer::load_or_create(&repo, None).unwrap();
        let exchange = |issuer: &mut SealIssuer, tag: &str| {
            issuer
                .emit(
                    SealSubject {
                        input: tag.as_bytes(),
                        output: tag.as_bytes(),
                        modality: "text",
                    },
                    SealGenerator {
                        id: "openai/gpt-4o",
                        version: None,
                        params: vec![],
                    },
                )
                .unwrap()
        };
        let s0 = exchange(&mut issuer, "req 0");
        let bundle = issue(&mut issuer, AUDIT.as_bytes(), &binding()).unwrap();
        let s2 = exchange(&mut issuer, "req 2");

        assert_eq!(s0["chain"]["sequence"], 0);
        assert_eq!(bundle["seal"]["chain"]["sequence"], 1);
        assert_eq!(s2["chain"]["sequence"], 2);
        let s0_hash = format!(
            "sha256:{}",
            seal::sha256_hex(&seal::signing_payload(&s0).unwrap())
        );
        assert_eq!(bundle["seal"]["chain"]["prev_seal_hash"], s0_hash);
        assert_eq!(seal::verify_chain(&repo).unwrap(), 3);
        assert_eq!(verify_bundle(&bundle).unwrap().sequence, 1);
    }

    #[test]
    fn repo_urls_lose_their_credentials() {
        assert_eq!(
            sanitize_repo_url("https://x-access-token:ghs_abc@github.com/o/r"),
            "https://github.com/o/r"
        );
        assert_eq!(
            sanitize_repo_url("https://user@github.com/o/r.git"),
            "https://github.com/o/r.git"
        );
        assert_eq!(
            sanitize_repo_url("https://github.com/o/r"),
            "https://github.com/o/r"
        );
        assert_eq!(
            sanitize_repo_url("git@github.com:o/r.git"),
            "git@github.com:o/r.git"
        );
        assert_eq!(sanitize_repo_url("ssh://git@host/o/r"), "ssh://host/o/r");
        let tmp = tempfile::tempdir().unwrap();
        let label = repo_label(tmp.path());
        assert!(label.starts_with("sha256:") && label.len() == 71, "{label}");
    }

    #[test]
    fn input_object_is_csc1_canonical_and_stable() {
        let bytes = seal::csc1_serialize(&binding().input_value()).unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            format!(
                "{{\"commit\":\"{}\",\"method\":\"v2\",\"options\":{{\"allow_shallow\":false}},\"type\":\"{INPUT_TYPE}\"}}",
                "a".repeat(40)
            )
        );
    }
}
