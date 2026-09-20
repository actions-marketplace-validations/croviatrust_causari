use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::repo::Repo;

// The capture layer is what makes Causari's provenance *real* instead of
// self-reported. Two independent streams flow into `.causari/capture/`:
//
// - `exchanges.jsonl` — every LLM request/response seen by `re proxy`
// - `prompts.jsonl`   — every user prompt reported by agent hooks (`re hook`)
//
// `re watch` then performs the **causal join**: when files change on disk,
// the inserted lines are searched *inside* recent completions. If the code
// that appeared in a file also appeared in a model's answer moments before,
// the two are causally linked — prompt, model, tokens and cost get attached
// to the filesystem event. No agent cooperation required.
//
// The Claude Code hook (`re hook-event post-tool`) runs the same join in the
// other direction: it knows the file and the prompt exactly, and borrows
// model, tokens and cost from the one recent Claude exchange whose
// completion contains the lines it just wrote.

/// A single LLM request/response captured by `re proxy`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exchange {
    /// Unique capture id (128 random bits, hex), assigned by the proxy.
    /// Two parallel deterministic calls can finish in the same millisecond
    /// with identical prompt and completion; they are still two billable
    /// exchanges and must be attributable separately. Absent on lines
    /// written by older binaries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Unix epoch milliseconds when the response completed.
    pub ts_ms: u64,
    /// Best-effort agent identity (from the User-Agent header).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// The model that served the call: the response's `model` field when the
    /// provider sends one (a dated snapshot behind an alias), otherwise the
    /// model the client requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The last user message in the request (the task that drove the call).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// The completion as join material: the assistant's plain text, then the
    /// string values of every tool call it made (OpenAI `tool_calls`
    /// arguments, Anthropic `tool_use` input, Responses `function_call`
    /// arguments) — file paths and file contents included. Assembled from
    /// SSE deltas when streaming. Reasoning, images and audio are not kept.
    #[serde(default)]
    pub response_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// SHA-256 of the exact request bytes sent upstream. Lets a seal, a
    /// transcript or a third party be matched to this exchange without the
    /// bytes themselves ever being stored. Absent on older lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_sha256: Option<String>,
    /// SHA-256 of the exact response bytes returned to the client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_sha256: Option<String>,
    /// `seal_id` of the crovia.seal.v1 receipt emitted for this exchange by
    /// `re proxy --seal`, when one was. This is the link from a completion
    /// to its receipt; without it seals are orphans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seal_id: Option<String>,
    /// The client went away before the response was fully relayed.
    /// `response_text`, tokens and cost cover only the bytes captured up to
    /// that point; the provider still billed the whole completion.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

/// A user prompt reported by an agent-side hook (e.g. Claude Code).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptRecord {
    pub ts_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub prompt: String,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn capture_dir(repo: &Repo) -> PathBuf {
    repo.dir.join("capture")
}

pub fn exchanges_path(repo: &Repo) -> PathBuf {
    capture_dir(repo).join("exchanges.jsonl")
}

pub fn prompts_path(repo: &Repo) -> PathBuf {
    capture_dir(repo).join("prompts.jsonl")
}

/// Ledger of exchanges already attributed to an event. One exchange's
/// tokens and dollars must land on exactly one event: without this, every
/// debounce window inside `--window` re-matched the same completion and
/// `re cost` multiplied the spend by the number of saves.
pub fn claims_path(repo: &Repo) -> PathBuf {
    capture_dir(repo).join("claims.jsonl")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeClaim {
    pub exchange: String,
    pub event: String,
    pub ts_ms: u64,
}

/// Fresh capture id for a new exchange.
pub fn new_exchange_id() -> Result<String> {
    let mut raw = [0u8; 16];
    getrandom::fill(&mut raw)
        .map_err(|e| anyhow::anyhow!("secure randomness unavailable: {}", e))?;
    Ok(hex::encode(raw))
}

/// Identity used by the claims ledger. The capture id when present;
/// otherwise (legacy lines) a digest of timestamp, prompt and completion.
pub fn exchange_key(e: &Exchange) -> String {
    if let Some(id) = &e.id {
        return format!("id:{}", id);
    }
    let mut h = blake3::Hasher::new();
    h.update(&e.ts_ms.to_le_bytes());
    h.update(e.prompt.as_deref().unwrap_or("").as_bytes());
    h.update(&[0]);
    h.update(e.response_text.as_bytes());
    h.finalize().to_hex()[..32].to_string()
}

pub fn load_claimed(repo: &Repo) -> Result<std::collections::HashSet<String>> {
    let path = claims_path(repo);
    if !path.exists() {
        return Ok(Default::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(raw
        .lines()
        .filter_map(|l| serde_json::from_str::<ExchangeClaim>(l).ok())
        .map(|c| c.exchange)
        .collect())
}

pub fn claim_exchange(repo: &Repo, e: &Exchange, event_id: &str) -> Result<()> {
    append_jsonl(
        &claims_path(repo),
        &ExchangeClaim {
            exchange: exchange_key(e),
            event: event_id.to_string(),
            ts_ms: now_ms(),
        },
    )
}

/// Exchanges since `since_ms` that have not yet been attributed to an event.
pub fn load_unclaimed_exchanges_since(repo: &Repo, since_ms: u64) -> Result<Vec<Exchange>> {
    let claimed = load_claimed(repo)?;
    Ok(load_exchanges_since(repo, since_ms)?
        .into_iter()
        .filter(|e| !claimed.contains(&exchange_key(e)))
        .collect())
}

/// Append one JSON object as a line to an append-only ledger file.
pub fn append_jsonl<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    let line = serde_json::to_string(value)?;
    writeln!(f, "{}", line)?;
    Ok(())
}

/// Load all exchanges captured at or after `since_ms`.
pub fn load_exchanges_since(repo: &Repo, since_ms: u64) -> Result<Vec<Exchange>> {
    let path = exchanges_path(repo);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(raw
        .lines()
        .filter_map(|l| serde_json::from_str::<Exchange>(l).ok())
        .filter(|e| e.ts_ms >= since_ms)
        .collect())
}

/// Most recent hook-reported prompt, optionally restricted to a session.
pub fn last_prompt(repo: &Repo, session_id: Option<&str>) -> Result<Option<PromptRecord>> {
    let path = prompts_path(repo);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(raw
        .lines()
        .filter_map(|l| serde_json::from_str::<PromptRecord>(l).ok())
        .rfind(|p| match session_id {
            Some(sid) => p.session_id.as_deref() == Some(sid),
            None => true,
        }))
}

// ---------------------------------------------------------------------------
// The correlation engine: content-based causal join
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Correlation {
    pub exchange: Exchange,
    /// Fraction of significant inserted lines found inside the completion.
    pub score: f64,
    pub matched: usize,
    pub considered: usize,
}

/// Minimum trimmed length for a line to count as "significant".
/// Filters out braces, blank-ish lines and one-token noise that would
/// match any completion by accident.
const MIN_SIGNIFICANT_LEN: usize = 6;

/// Minimum score for a correlation to be trusted.
const MIN_SCORE: f64 = 0.25;

/// Given the lines inserted by a filesystem change and the recent LLM
/// exchanges, find the exchange that most plausibly *produced* the change.
///
/// The join is by **content**, not just time: each significant inserted line
/// is searched verbatim inside each completion. Code that an agent wrote to
/// disk almost always appeared first in a model response — that overlap is
/// the causal fingerprint.
pub fn correlate(added_lines: &[String], exchanges: &[Exchange]) -> Option<Correlation> {
    let considered = significant_lines(added_lines);
    if considered.is_empty() {
        return None;
    }
    let mut best: Option<Correlation> = None;
    for ex in exchanges {
        let matched = count_contained(&considered, &ex.response_text);
        if matched == 0 {
            continue;
        }
        let score = matched as f64 / considered.len() as f64;
        let better = match &best {
            None => true,
            Some(b) => score > b.score || (score == b.score && ex.ts_ms > b.exchange.ts_ms),
        };
        if better {
            best = Some(Correlation {
                exchange: ex.clone(),
                score,
                matched,
                considered: considered.len(),
            });
        }
    }
    best.filter(|b| b.score >= MIN_SCORE)
}

/// The inserted lines worth matching: trimmed and long enough not to occur
/// in any completion by accident.
pub fn significant_lines(added_lines: &[String]) -> Vec<&str> {
    added_lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| l.len() >= MIN_SIGNIFICANT_LEN)
        .collect()
}

/// How many of `lines` occur verbatim inside `text`.
pub fn count_contained(lines: &[&str], text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    lines.iter().filter(|l| text.contains(**l)).count()
}

/// Does the overlap clear the same bar `correlate` applies?
pub fn overlap_is_significant(matched: usize, considered: usize) -> bool {
    considered > 0 && matched as f64 / considered as f64 >= MIN_SCORE
}

// ---------------------------------------------------------------------------
// Request / response parsing (OpenAI + Anthropic wire formats)
// ---------------------------------------------------------------------------

/// Extract the last *user* message from an OpenAI `messages` array, an
/// Anthropic `messages` array, or an OpenAI Responses `input` array.
pub fn extract_prompt(body: &Value) -> Option<String> {
    let messages = body
        .get("messages")
        .or_else(|| body.get("input"))?
        .as_array()?;
    for m in messages.iter().rev() {
        if m.get("role").and_then(|r| r.as_str()) != Some("user") {
            continue;
        }
        if let Some(text) = m.get("content").and_then(content_text) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Flatten a content field: plain string, or array of text blocks.
fn content_text(c: &Value) -> Option<String> {
    match c {
        Value::String(s) => Some(s.clone()),
        Value::Array(parts) => {
            let mut out = String::new();
            for p in parts {
                let is_text = p
                    .get("type")
                    .and_then(|t| t.as_str())
                    .map(|t| t == "text" || t == "input_text")
                    .unwrap_or(true);
                if !is_text {
                    continue;
                }
                if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(t);
                }
            }
            if out.is_empty() { None } else { Some(out) }
        }
        _ => None,
    }
}

/// What the capture layer extracts from one completion, whatever the wire
/// shape. `text` is the join target for `correlate`: plain assistant text
/// first, then every string value found inside tool-call arguments — that
/// is where coding agents put the code they write to disk, and a completion
/// that is *only* a tool call used to leave `response_text` empty.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedResponse {
    pub text: String,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    /// The model the provider actually served (`model` in the response),
    /// often more specific than the alias the client asked for.
    pub model: Option<String>,
}

/// Append a segment to the join text, newline-separated so the last line
/// of one segment and the first of the next are never fused into a line
/// no file could contain.
fn push_segment(out: &mut String, s: &str) {
    if s.is_empty() {
        return;
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(s);
}

/// Every string leaf of a JSON value, depth-first. Tool inputs look like
/// `{"file_path": "...", "content": "..."}`: the keys are the tool's
/// vocabulary, the string values are what lands in files.
fn push_string_leaves(out: &mut String, v: &Value) {
    match v {
        Value::String(s) => push_segment(out, s),
        Value::Array(items) => items.iter().for_each(|x| push_string_leaves(out, x)),
        Value::Object(map) => map.values().for_each(|x| push_string_leaves(out, x)),
        _ => {}
    }
}

/// Tool-call arguments travel as a JSON-encoded string. Decode it and take
/// the string leaves; a payload that is not valid JSON (a truncated stream,
/// a custom tool with a raw-text protocol) is kept verbatim, not dropped.
fn push_arguments(out: &mut String, raw: &str) {
    match serde_json::from_str::<Value>(raw) {
        Ok(v) => push_string_leaves(out, &v),
        Err(_) => push_segment(out, raw),
    }
}

/// OpenAI names the pair `prompt_tokens`/`completion_tokens`; Anthropic and
/// the Responses API `input_tokens`/`output_tokens`. Later readings win, so
/// a final cumulative usage block overrides an early partial one.
fn read_usage(u: &Value, p: &mut ParsedResponse) {
    if let Some(x) = u
        .get("prompt_tokens")
        .or_else(|| u.get("input_tokens"))
        .and_then(Value::as_u64)
    {
        p.tokens_in = Some(x);
    }
    if let Some(x) = u
        .get("completion_tokens")
        .or_else(|| u.get("output_tokens"))
        .and_then(Value::as_u64)
    {
        p.tokens_out = Some(x);
    }
}

fn read_model(v: &Value, p: &mut ParsedResponse) {
    if let Some(m) = v.get("model").and_then(Value::as_str) {
        if !m.is_empty() {
            p.model = Some(m.to_string());
        }
    }
}

/// Tool-call arguments arrive over a stream as fragments spread across many
/// events, interleaved between calls. They only parse once reassembled per
/// call, in the order the calls started.
#[derive(Default)]
struct ArgBuffers(Vec<(String, String)>);

impl ArgBuffers {
    fn slot(&mut self, key: String) -> &mut String {
        if let Some(pos) = self.0.iter().position(|(k, _)| *k == key) {
            &mut self.0[pos].1
        } else {
            self.0.push((key, String::new()));
            &mut self.0.last_mut().expect("just pushed").1
        }
    }
    fn append(&mut self, key: String, fragment: &str) {
        self.slot(key).push_str(fragment);
    }
    /// A `*.done` event carries the complete payload; it replaces whatever
    /// fragments were collected so nothing is counted twice.
    fn set(&mut self, key: String, full: &str) {
        *self.slot(key) = full.to_string();
    }
    fn flush_into(self, out: &mut String) {
        for (_, raw) in self.0 {
            push_arguments(out, &raw);
        }
    }
}

/// Parse a non-streaming completion: OpenAI chat (`choices[].message`),
/// Anthropic messages (`content[]`) or OpenAI Responses (`output[]`).
/// Text and tool-call payloads both end up in `text`.
pub fn parse_response_json(v: &Value) -> ParsedResponse {
    let mut p = ParsedResponse::default();
    // OpenAI chat: message.content, then message.tool_calls[].function.arguments
    for ch in v
        .get("choices")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(msg) = ch.get("message") else {
            continue;
        };
        if let Some(c) = msg.get("content").and_then(Value::as_str) {
            push_segment(&mut p.text, c);
        }
        for call in msg
            .get("tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(args) = call.pointer("/function/arguments").and_then(Value::as_str) {
                push_arguments(&mut p.text, args);
            }
        }
    }
    // Anthropic: content[] text blocks and tool_use inputs
    for block in v
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(t) = block.get("text").and_then(Value::as_str) {
                    push_segment(&mut p.text, t);
                }
            }
            Some("tool_use") => {
                if let Some(input) = block.get("input") {
                    push_string_leaves(&mut p.text, input);
                }
            }
            _ => {}
        }
    }
    // Responses API: output[] items (reasoning items carry no text)
    for item in v
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                for part in item
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if part.get("type").and_then(Value::as_str) == Some("output_text") {
                        if let Some(t) = part.get("text").and_then(Value::as_str) {
                            push_segment(&mut p.text, t);
                        }
                    }
                }
            }
            Some("function_call") => {
                if let Some(a) = item.get("arguments").and_then(Value::as_str) {
                    push_arguments(&mut p.text, a);
                }
            }
            // Codex's `apply_patch` freeform tool: the patch is the input.
            Some("custom_tool_call") => {
                if let Some(i) = item.get("input").and_then(Value::as_str) {
                    push_segment(&mut p.text, i);
                }
            }
            _ => {}
        }
    }
    if let Some(u) = v.get("usage") {
        read_usage(u, &mut p);
    }
    read_model(v, &mut p);
    p
}

/// Assemble text, tool-call payloads, usage and model from a captured SSE
/// stream. Handles OpenAI chat chunks (`delta.content`, `delta.tool_calls`),
/// Anthropic events (`content_block_delta` with `text_delta` /
/// `input_json_delta`, `message_start`, `message_delta`) and Responses API
/// events (`response.output_text.delta`,
/// `response.function_call_arguments.delta`, `response.completed`).
pub fn parse_sse(body: &str) -> ParsedResponse {
    let mut p = ParsedResponse::default();
    let mut args = ArgBuffers::default();
    for line in body.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        // OpenAI chat: choices[].delta.{content, tool_calls[].function.arguments}
        for (ci, ch) in v
            .get("choices")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let Some(delta) = ch.get("delta") else {
                continue;
            };
            if let Some(c) = delta.get("content").and_then(Value::as_str) {
                p.text.push_str(c);
            }
            for call in delta
                .get("tool_calls")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let idx = call.get("index").and_then(Value::as_u64).unwrap_or(0);
                let key = format!("openai:{ci}:{idx}");
                if let Some(frag) = call.pointer("/function/arguments").and_then(Value::as_str) {
                    args.append(key, frag);
                } else {
                    args.slot(key);
                }
            }
        }
        match v.get("type").and_then(Value::as_str) {
            // Anthropic
            Some("message_start") => {
                if let Some(m) = v.get("message") {
                    if let Some(u) = m.get("usage") {
                        read_usage(u, &mut p);
                    }
                    read_model(m, &mut p);
                }
            }
            Some("content_block_start") => {
                if v.pointer("/content_block/type").and_then(Value::as_str) == Some("tool_use") {
                    let idx = v.get("index").and_then(Value::as_u64).unwrap_or(0);
                    args.slot(format!("anthropic:{idx}"));
                }
            }
            Some("content_block_delta") => {
                let idx = v.get("index").and_then(Value::as_u64).unwrap_or(0);
                if let Some(d) = v.get("delta") {
                    if let Some(t) = d.get("text").and_then(Value::as_str) {
                        p.text.push_str(t);
                    }
                    if let Some(j) = d.get("partial_json").and_then(Value::as_str) {
                        args.append(format!("anthropic:{idx}"), j);
                    }
                }
            }
            // Responses API
            Some("response.output_text.delta") => {
                if let Some(d) = v.get("delta").and_then(Value::as_str) {
                    p.text.push_str(d);
                }
            }
            Some("response.function_call_arguments.delta")
            | Some("response.custom_tool_call_input.delta") => {
                if let Some(d) = v.get("delta").and_then(Value::as_str) {
                    args.append(responses_item_key(&v), d);
                }
            }
            Some("response.function_call_arguments.done") => {
                if let Some(a) = v.get("arguments").and_then(Value::as_str) {
                    args.set(responses_item_key(&v), a);
                }
            }
            Some("response.custom_tool_call_input.done") => {
                if let Some(i) = v.get("input").and_then(Value::as_str) {
                    args.set(responses_item_key(&v), i);
                }
            }
            Some("response.created") | Some("response.completed") | Some("response.incomplete") => {
                if let Some(r) = v.get("response") {
                    if let Some(u) = r.get("usage") {
                        read_usage(u, &mut p);
                    }
                    read_model(r, &mut p);
                }
            }
            _ => {}
        }
        // Top-level usage: the final OpenAI chunk (with
        // `stream_options.include_usage`) and Anthropic `message_delta`.
        if let Some(u) = v.get("usage") {
            read_usage(u, &mut p);
        }
        // Every OpenAI chat chunk names the served model.
        read_model(&v, &mut p);
    }
    args.flush_into(&mut p.text);
    p
}

/// One Responses output item is identified by `item_id`; `output_index` is
/// the fallback for servers that omit it.
fn responses_item_key(v: &Value) -> String {
    match v.get("item_id").and_then(Value::as_str) {
        Some(id) => format!("responses:{id}"),
        None => format!(
            "responses:#{}",
            v.get("output_index").and_then(Value::as_u64).unwrap_or(0)
        ),
    }
}

// ---------------------------------------------------------------------------
// Cost estimation (best effort)
// ---------------------------------------------------------------------------

/// USD per 1M tokens (input, output). Prefix-matched against the model id.
/// Best-effort defaults; exact billing always belongs to the provider.
/// More specific prefixes must come before less specific ones.
const PRICES_PER_MTOK: &[(&str, f64, f64)] = &[
    ("gpt-4o-mini", 0.15, 0.60),
    ("gpt-4o", 2.50, 10.00),
    ("gpt-4.1-mini", 0.40, 1.60),
    ("gpt-4.1-nano", 0.10, 0.40),
    ("gpt-4.1", 2.00, 8.00),
    ("o4-mini", 1.10, 4.40),
    ("o3", 2.00, 8.00),
    ("claude-3-5-haiku", 0.80, 4.00),
    ("claude-3-5-sonnet", 3.00, 15.00),
    ("claude-haiku", 1.00, 5.00),
    ("claude-sonnet", 3.00, 15.00),
    ("claude-opus", 15.00, 75.00),
];

pub fn estimate_cost(model: Option<&str>, tin: Option<u64>, tout: Option<u64>) -> Option<f64> {
    let model = model?;
    let (pin, pout) = PRICES_PER_MTOK
        .iter()
        .find(|(prefix, _, _)| model.starts_with(prefix) || model.contains(prefix))
        .map(|(_, i, o)| (*i, *o))?;
    if tin.is_none() && tout.is_none() {
        return None;
    }
    let tin = tin.unwrap_or(0) as f64;
    let tout = tout.unwrap_or(0) as f64;
    Some((tin * pin + tout * pout) / 1_000_000.0)
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ex(ts_ms: u64, text: &str) -> Exchange {
        Exchange {
            id: None,
            ts_ms,
            agent: None,
            model: Some("gpt-4o".into()),
            prompt: Some("fix the bug".into()),
            response_text: text.into(),
            tokens_in: Some(100),
            tokens_out: Some(50),
            cost_usd: None,
            request_sha256: None,
            response_sha256: None,
            seal_id: None,
            truncated: false,
        }
    }

    #[test]
    fn correlate_finds_content_match() {
        let added = vec![
            "fn refresh_token(user: &User) -> Result<Token> {".to_string(),
            "    rotate_every(Duration::hours(24))".to_string(),
            "}".to_string(), // insignificant, filtered out
        ];
        let exchanges = vec![
            ex(1_000, "unrelated chatter about the weather"),
            ex(
                2_000,
                "Here is the fix:\n```rust\nfn refresh_token(user: &User) -> Result<Token> {\n    rotate_every(Duration::hours(24))\n}\n```",
            ),
        ];
        let c = correlate(&added, &exchanges).expect("must correlate");
        assert_eq!(c.exchange.ts_ms, 2_000);
        assert_eq!(c.matched, 2);
        assert_eq!(c.considered, 2);
        assert!((c.score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn correlate_rejects_weak_matches() {
        let added = vec![
            "let alpha = compute_alpha(input);".to_string(),
            "let beta = compute_beta(input);".to_string(),
            "let gamma = compute_gamma(input);".to_string(),
            "let delta = compute_delta(input);".to_string(),
            "let epsilon = compute_epsilon(input);".to_string(),
        ];
        // Only 1/5 lines present -> score 0.2 < 0.25 threshold.
        let exchanges = vec![ex(1_000, "let alpha = compute_alpha(input);")];
        assert!(correlate(&added, &exchanges).is_none());
    }

    #[test]
    fn claimed_exchange_is_attributed_only_once() {
        // Regression (F12): the same completion was re-correlated by every
        // watch window inside `--window`, multiplying tokens and cost.
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        let a = ex(1_000, "first completion body");
        let b = ex(2_000, "second completion body");
        append_jsonl(&exchanges_path(&repo), &a).unwrap();
        append_jsonl(&exchanges_path(&repo), &b).unwrap();
        assert_eq!(load_unclaimed_exchanges_since(&repo, 0).unwrap().len(), 2);

        claim_exchange(&repo, &a, "evt-1").unwrap();
        let left = load_unclaimed_exchanges_since(&repo, 0).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].ts_ms, 2_000);

        // Same timestamp, different content: a different exchange.
        assert_ne!(exchange_key(&a), exchange_key(&ex(1_000, "other")));

        // Two parallel calls, same ms, same prompt, same completion: with
        // capture ids they are still two exchanges and claiming one leaves
        // the other attributable.
        let mut c1 = ex(3_000, "identical");
        let mut c2 = ex(3_000, "identical");
        assert_eq!(
            exchange_key(&c1),
            exchange_key(&c2),
            "legacy fallback collides"
        );
        c1.id = Some(new_exchange_id().unwrap());
        c2.id = Some(new_exchange_id().unwrap());
        assert_ne!(exchange_key(&c1), exchange_key(&c2));
        append_jsonl(&exchanges_path(&repo), &c1).unwrap();
        append_jsonl(&exchanges_path(&repo), &c2).unwrap();
        claim_exchange(&repo, &c1, "evt-2").unwrap();
        let left = load_unclaimed_exchanges_since(&repo, 3_000).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, c2.id);
    }

    #[test]
    fn correlate_ignores_trivial_lines() {
        let added = vec!["}".to_string(), "{".to_string(), "  ".to_string()];
        let exchanges = vec![ex(1_000, "} { ")];
        assert!(correlate(&added, &exchanges).is_none());
    }

    #[test]
    fn extract_prompt_openai_string_content() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "You are helpful."},
                {"role": "user", "content": "Add JWT refresh logic"},
                {"role": "assistant", "content": "ok"},
                {"role": "user", "content": "rotate every 24h"}
            ]
        });
        assert_eq!(extract_prompt(&body).as_deref(), Some("rotate every 24h"));
    }

    #[test]
    fn extract_prompt_anthropic_block_content() {
        let body = json!({
            "model": "claude-sonnet-4",
            "messages": [
                {"role": "user", "content": [
                    {"type": "text", "text": "Fix the failing tests"},
                    {"type": "image", "source": {}}
                ]}
            ]
        });
        assert_eq!(
            extract_prompt(&body).as_deref(),
            Some("Fix the failing tests")
        );
    }

    #[test]
    fn parse_response_json_openai() {
        let v = json!({
            "model": "gpt-4o-2024-08-06",
            "choices": [{"message": {"role": "assistant", "content": "hello world"}}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 4}
        });
        let p = parse_response_json(&v);
        assert_eq!(p.text, "hello world");
        assert_eq!(p.tokens_in, Some(10));
        assert_eq!(p.tokens_out, Some(4));
        assert_eq!(p.model.as_deref(), Some("gpt-4o-2024-08-06"));
    }

    #[test]
    fn parse_response_json_anthropic() {
        let v = json!({
            "content": [{"type": "text", "text": "ciao"}],
            "usage": {"input_tokens": 7, "output_tokens": 2}
        });
        let p = parse_response_json(&v);
        assert_eq!(p.text, "ciao");
        assert_eq!(p.tokens_in, Some(7));
        assert_eq!(p.tokens_out, Some(2));
        assert_eq!(p.model, None);
    }

    #[test]
    fn parse_sse_openai_stream() {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"hel\"}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n\
                    data: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2}}\n\n\
                    data: [DONE]\n";
        let p = parse_sse(body);
        assert_eq!(p.text, "hello");
        assert_eq!(p.tokens_in, Some(5));
        assert_eq!(p.tokens_out, Some(2));
    }

    #[test]
    fn parse_sse_anthropic_stream() {
        let body = "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":12}}}\n\n\
                    data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"ci\"}}\n\n\
                    data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"ao\"}}\n\n\
                    data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":2}}\n";
        let p = parse_sse(body);
        assert_eq!(p.text, "ciao");
        assert_eq!(p.tokens_in, Some(12));
        assert_eq!(p.tokens_out, Some(2));
    }

    // -- Tool-call payloads: the code a coding agent writes lives here, not
    // in the text channel. Each fixture mirrors a real provider response.

    const CODE: &str = "def f():\n    return 42\n";

    fn sse(events: &[(&str, Value)]) -> String {
        events
            .iter()
            .map(|(name, data)| {
                if name.is_empty() {
                    format!("data: {}\n\n", data)
                } else {
                    format!("event: {}\ndata: {}\n\n", name, data)
                }
            })
            .collect()
    }

    fn assert_code_lines_present(text: &str) {
        for line in CODE.lines() {
            assert!(text.contains(line), "missing {:?} in {:?}", line, text);
        }
    }

    #[test]
    fn openai_tool_calls_non_stream() {
        let args = serde_json::to_string(&json!({"path": "src/f.py", "content": CODE})).unwrap();
        let v = json!({
            "id": "chatcmpl-9x", "object": "chat.completion", "created": 1_726_840_000,
            "model": "gpt-4o-2024-08-06",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1", "type": "function",
                        "function": {"name": "write_file", "arguments": args}
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 120, "completion_tokens": 33, "total_tokens": 153}
        });
        let p = parse_response_json(&v);
        assert_code_lines_present(&p.text);
        assert!(p.text.contains("src/f.py"));
        assert_eq!((p.tokens_in, p.tokens_out), (Some(120), Some(33)));
        assert_eq!(p.model.as_deref(), Some("gpt-4o-2024-08-06"));
    }

    #[test]
    fn openai_tool_calls_stream() {
        // Two tool calls interleaved by index, arguments split mid-string,
        // usage in a trailing chunk with empty `choices`.
        let a = serde_json::to_string(&json!({"path": "src/f.py", "content": CODE})).unwrap();
        let (a1, a2) = a.split_at(a.len() / 2);
        let b = serde_json::to_string(&json!({"cmd": "pytest -q"})).unwrap();
        let chunk = |tool_calls: Value| {
            json!({
                "id": "chatcmpl-9x", "object": "chat.completion.chunk", "model": "gpt-4o-2024-08-06",
                "choices": [{"index": 0, "delta": {"tool_calls": tool_calls}, "finish_reason": null}]
            })
        };
        let body = sse(&[
            (
                "",
                json!({"id":"chatcmpl-9x","object":"chat.completion.chunk","model":"gpt-4o-2024-08-06",
                "choices":[{"index":0,"delta":{"role":"assistant","content":null},"finish_reason":null}]}),
            ),
            (
                "",
                chunk(
                    json!([{"index":0,"id":"call_1","type":"function","function":{"name":"write_file","arguments":""}}]),
                ),
            ),
            (
                "",
                chunk(
                    json!([{"index":1,"id":"call_2","type":"function","function":{"name":"shell","arguments":""}}]),
                ),
            ),
            ("", chunk(json!([{"index":0,"function":{"arguments":a1}}]))),
            ("", chunk(json!([{"index":1,"function":{"arguments":b}}]))),
            ("", chunk(json!([{"index":0,"function":{"arguments":a2}}]))),
            (
                "",
                json!({"id":"chatcmpl-9x","object":"chat.completion.chunk","model":"gpt-4o-2024-08-06",
                "choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
            ),
            (
                "",
                json!({"id":"chatcmpl-9x","object":"chat.completion.chunk","model":"gpt-4o-2024-08-06",
                "choices":[],"usage":{"prompt_tokens":120,"completion_tokens":40,"total_tokens":160}}),
            ),
        ]) + "data: [DONE]\n\n";
        let p = parse_sse(&body);
        assert_code_lines_present(&p.text);
        assert!(p.text.contains("pytest -q"));
        assert_eq!((p.tokens_in, p.tokens_out), (Some(120), Some(40)));
        assert_eq!(p.model.as_deref(), Some("gpt-4o-2024-08-06"));
    }

    #[test]
    fn anthropic_tool_use_non_stream() {
        let v = json!({
            "id": "msg_01", "type": "message", "role": "assistant",
            "model": "claude-sonnet-4-20250514",
            "content": [
                {"type": "text", "text": "I'll create the helper."},
                {"type": "tool_use", "id": "toolu_01", "name": "Write",
                 "input": {"file_path": "/repo/src/f.py", "content": CODE}}
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 900, "output_tokens": 80}
        });
        let p = parse_response_json(&v);
        assert!(p.text.starts_with("I'll create the helper."));
        assert_code_lines_present(&p.text);
        assert!(p.text.contains("/repo/src/f.py"));
        assert_eq!((p.tokens_in, p.tokens_out), (Some(900), Some(80)));
        assert_eq!(p.model.as_deref(), Some("claude-sonnet-4-20250514"));
    }

    #[test]
    fn anthropic_tool_use_stream() {
        let input = serde_json::to_string(&json!({"file_path": "/repo/src/f.py", "content": CODE}))
            .unwrap();
        let (j1, rest) = input.split_at(9);
        let (j2, j3) = rest.split_at(rest.len() / 2);
        let body = sse(&[
            (
                "message_start",
                json!({"type":"message_start","message":{"id":"msg_01","type":"message","role":"assistant",
                "model":"claude-sonnet-4-20250514","content":[],"stop_reason":null,
                "usage":{"input_tokens":900,"output_tokens":1}}}),
            ),
            (
                "content_block_start",
                json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            ),
            (
                "content_block_delta",
                json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"I'll create "}}),
            ),
            (
                "content_block_delta",
                json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"the helper."}}),
            ),
            (
                "content_block_stop",
                json!({"type":"content_block_stop","index":0}),
            ),
            (
                "content_block_start",
                json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_01","name":"Write","input":{}}}),
            ),
            (
                "content_block_delta",
                json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":j1}}),
            ),
            (
                "content_block_delta",
                json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":j2}}),
            ),
            (
                "content_block_delta",
                json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":j3}}),
            ),
            (
                "content_block_stop",
                json!({"type":"content_block_stop","index":1}),
            ),
            (
                "message_delta",
                json!({"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":80}}),
            ),
            ("message_stop", json!({"type":"message_stop"})),
        ]);
        let p = parse_sse(&body);
        assert!(p.text.starts_with("I'll create the helper."));
        assert_code_lines_present(&p.text);
        assert_eq!((p.tokens_in, p.tokens_out), (Some(900), Some(80)));
        assert_eq!(p.model.as_deref(), Some("claude-sonnet-4-20250514"));
    }

    #[test]
    fn responses_non_stream() {
        // Codex-style: reasoning item, a short message, and an apply_patch
        // function call whose patch body carries the code with `+` prefixes.
        let patch = format!(
            "*** Begin Patch\n*** Add File: src/f.py\n{}*** End Patch\n",
            CODE.lines().map(|l| format!("+{l}\n")).collect::<String>()
        );
        let args = serde_json::to_string(&json!({"patch": patch})).unwrap();
        let v = json!({
            "id": "resp_1", "object": "response", "model": "gpt-4.1-2025-04-14", "status": "completed",
            "output": [
                {"type": "reasoning", "id": "rs_1", "summary": []},
                {"type": "message", "id": "msg_1", "role": "assistant", "status": "completed",
                 "content": [{"type": "output_text", "text": "Adding f to src/f.py.", "annotations": []}]},
                {"type": "function_call", "id": "fc_1", "call_id": "call_1", "name": "apply_patch",
                 "arguments": args, "status": "completed"}
            ],
            "usage": {"input_tokens": 500, "output_tokens": 60, "total_tokens": 560}
        });
        let p = parse_response_json(&v);
        assert!(p.text.starts_with("Adding f to src/f.py."));
        assert_code_lines_present(&p.text);
        assert!(p.text.contains("*** Add File: src/f.py"));
        assert_eq!((p.tokens_in, p.tokens_out), (Some(500), Some(60)));
        assert_eq!(p.model.as_deref(), Some("gpt-4.1-2025-04-14"));
    }

    #[test]
    fn responses_stream() {
        let args = serde_json::to_string(&json!({"path": "src/f.py", "content": CODE})).unwrap();
        let (a1, a2) = args.split_at(args.len() / 3);
        let body = sse(&[
            (
                "response.created",
                json!({"type":"response.created","sequence_number":0,
                "response":{"id":"resp_1","object":"response","model":"gpt-4.1-2025-04-14","status":"in_progress","output":[],"usage":null}}),
            ),
            (
                "response.output_item.added",
                json!({"type":"response.output_item.added","output_index":0,
                "item":{"type":"message","id":"msg_1","role":"assistant","status":"in_progress","content":[]}}),
            ),
            (
                "response.output_text.delta",
                json!({"type":"response.output_text.delta","item_id":"msg_1","output_index":0,"content_index":0,"delta":"Adding "}),
            ),
            (
                "response.output_text.delta",
                json!({"type":"response.output_text.delta","item_id":"msg_1","output_index":0,"content_index":0,"delta":"f."}),
            ),
            (
                "response.output_text.done",
                json!({"type":"response.output_text.done","item_id":"msg_1","output_index":0,"content_index":0,"text":"Adding f."}),
            ),
            (
                "response.output_item.added",
                json!({"type":"response.output_item.added","output_index":1,
                "item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"write_file","arguments":"","status":"in_progress"}}),
            ),
            (
                "response.function_call_arguments.delta",
                json!({"type":"response.function_call_arguments.delta","item_id":"fc_1","output_index":1,"delta":a1}),
            ),
            (
                "response.function_call_arguments.delta",
                json!({"type":"response.function_call_arguments.delta","item_id":"fc_1","output_index":1,"delta":a2}),
            ),
            (
                "response.function_call_arguments.done",
                json!({"type":"response.function_call_arguments.done","item_id":"fc_1","output_index":1,"arguments":args}),
            ),
            (
                "response.completed",
                json!({"type":"response.completed",
                "response":{"id":"resp_1","object":"response","model":"gpt-4.1-2025-04-14","status":"completed",
                    "usage":{"input_tokens":500,"output_tokens":60,"total_tokens":560}}}),
            ),
        ]);
        let p = parse_sse(&body);
        assert!(p.text.starts_with("Adding f."));
        assert_code_lines_present(&p.text);
        // `.done` replaced the fragments: the code appears exactly once.
        assert_eq!(p.text.matches("def f():").count(), 1);
        assert_eq!((p.tokens_in, p.tokens_out), (Some(500), Some(60)));
        assert_eq!(p.model.as_deref(), Some("gpt-4.1-2025-04-14"));
    }

    #[test]
    fn unparseable_arguments_are_kept_verbatim() {
        let v = json!({
            "choices": [{"message": {"role": "assistant", "tool_calls": [{
                "id": "call_1", "type": "function",
                "function": {"name": "apply_patch", "arguments": "*** Begin Patch\n+def f():\n*** End Patch"}
            }]}}]
        });
        let p = parse_response_json(&v);
        assert!(p.text.contains("+def f():"));
    }

    #[test]
    fn estimate_cost_prefix_match() {
        let c = estimate_cost(Some("gpt-4o-2024-11-20"), Some(1_000_000), Some(1_000_000));
        assert!((c.unwrap() - 12.50).abs() < 1e-9);
        assert!(estimate_cost(Some("unknown-model"), Some(10), Some(10)).is_none());
        assert!(estimate_cost(None, Some(10), Some(10)).is_none());
    }
}
