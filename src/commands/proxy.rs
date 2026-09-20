use anyhow::{Result, anyhow};
use colored::Colorize;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::capture::{
    Exchange, ParsedResponse, append_jsonl, estimate_cost, exchanges_path, extract_prompt, now_ms,
    parse_response_json, parse_sse,
};
use crate::cli::ProxyArgs;
use crate::repo::Repo;
use crate::seal::{SealGenerator, SealIssuer, SealSubject};

/// `re proxy` — the heart of the capture layer.
///
/// A local, single-binary LLM proxy. Point any agent at it
/// (`OPENAI_BASE_URL` / `ANTHROPIC_BASE_URL`) and every prompt, completion,
/// token count and dollar flows through Causari on its way to the provider.
/// Bytes are streamed to the client in real time (tee capture), so streaming
/// agents feel no difference.
///
/// Captured exchanges land in `.causari/capture/exchanges.jsonl`, where
/// `re watch` joins them with filesystem changes by *content*: the lines that
/// appear in your files are searched inside the completions that preceded
/// them. That join is what turns "12 files changed" into "12 files changed
/// because this prompt asked this model, and it cost $0.14".
pub fn run(args: ProxyArgs) -> Result<()> {
    let repo = Arc::new(Repo::discover()?);
    let port = args.port.unwrap_or(4242);
    let sealer = if args.seal {
        let issuer = SealIssuer::load_or_create(&repo, args.seal_issuer.clone())?;
        println!(
            "{} Crovia Seal issuer active — pubkey {}",
            "causari:".green().bold(),
            issuer.pubkey_hex().bright_white()
        );
        Some(Mutex::new(issuer))
    } else {
        None
    };
    let cfg = Arc::new(ProxyConfig {
        openai: args
            .openai_upstream
            .unwrap_or_else(|| "https://api.openai.com".to_string()),
        anthropic: args
            .anthropic_upstream
            .unwrap_or_else(|| "https://api.anthropic.com".to_string()),
        sealer,
    });

    let server = Server::http(("127.0.0.1", port))
        .map_err(|e| anyhow!("cannot bind 127.0.0.1:{}: {}", port, e))?;

    println!(
        "{} LLM capture proxy listening on {}",
        "causari:".green().bold(),
        format!("http://127.0.0.1:{}", port).cyan()
    );
    println!();
    println!("  Point your agent at it:");
    println!(
        "    {}  {}",
        "OPENAI_BASE_URL".bright_black(),
        format!("http://127.0.0.1:{}/openai/v1", port).bright_white()
    );
    println!(
        "    {}  {}",
        "ANTHROPIC_BASE_URL".bright_black(),
        format!("http://127.0.0.1:{}/anthropic", port).bright_white()
    );
    println!();
    println!(
        "  Captures to {} — run {} in another terminal to join captures with file changes.",
        ".causari/capture/exchanges.jsonl".bright_black(),
        "re watch".cyan()
    );
    println!(
        "  {} OpenAI chat requests with {} get {} added, so streamed completions carry tokens and cost.",
        "usage requested on streams:".bright_black(),
        "stream:true".bright_black(),
        "stream_options.include_usage".bright_black()
    );
    println!("  Press Ctrl-C to stop.");
    println!();

    for request in server.incoming_requests() {
        let cfg = Arc::clone(&cfg);
        let repo = Arc::clone(&repo);
        std::thread::spawn(move || {
            if let Err(e) = handle(request, &cfg, &repo) {
                eprintln!("{} {}", "proxy error:".red(), e);
            }
        });
    }
    Ok(())
}

struct ProxyConfig {
    openai: String,
    anthropic: String,
    /// When set, every completion also produces a Crovia Seal
    /// (draft-crovia-seal-01): an Ed25519-signed, hash-chained receipt.
    /// Mutex because the chain state (sequence, prev hash) is strictly serial.
    sealer: Option<Mutex<SealIssuer>>,
}

/// Generation parameters worth committing into the seal, stringified per
/// CSC-1 (floats are forbidden in signed payloads).
fn seal_params(body: Option<&serde_json::Value>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(obj) = body.and_then(|v| v.as_object()) {
        for key in ["temperature", "top_p", "max_tokens", "max_output_tokens"] {
            if let Some(v) = obj.get(key) {
                let s = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                out.push((key.to_string(), s));
            }
        }
    }
    out
}

/// Map an incoming path to (upstream_base, upstream_path).
/// Explicit prefixes win; bare Anthropic/OpenAI paths fall through for
/// drop-in compatibility with clients that only allow a host override.
fn route(url: &str, cfg: &ProxyConfig) -> (String, String) {
    if let Some(rest) = url.strip_prefix("/anthropic") {
        (cfg.anthropic.clone(), rest.to_string())
    } else if let Some(rest) = url.strip_prefix("/openai") {
        (cfg.openai.clone(), rest.to_string())
    } else if url.starts_with("/v1/messages") {
        (cfg.anthropic.clone(), url.to_string())
    } else {
        (cfg.openai.clone(), url.to_string())
    }
}

/// Requests whose response is a model completion worth capturing: a POST
/// to a completion endpoint. Substring matching used to record
/// `/v1/messages/count_tokens` (no completion, no usage) and
/// `GET /v1/responses/{id}` (a replay of a completion already captured)
/// as exchanges of their own.
fn is_completion_request(method: &Method, path: &str) -> bool {
    if *method != Method::Post {
        return false;
    }
    let path = endpoint(path);
    ["/chat/completions", "/messages", "/responses"]
        .iter()
        .any(|suffix| path.ends_with(suffix))
}

/// The path without query string, fragment or trailing slash.
fn endpoint(path: &str) -> &str {
    let path = path.split(['?', '#']).next().unwrap_or("");
    path.strip_suffix('/').unwrap_or(path)
}

/// OpenAI reports usage on a stream only when the client asks for it with
/// `stream_options.include_usage`; most agents do not, so every streamed
/// chat completion was captured with no tokens and no cost. Returns the
/// request body with that option set when it applies (a streaming chat
/// completion with a JSON object body), `None` when the body is forwarded
/// untouched. The one extra terminal chunk (empty `choices`, `usage`) is
/// part of the documented protocol and is passed through to the client.
fn with_stream_usage(body: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    if !endpoint(path).ends_with("/chat/completions") {
        return None;
    }
    let mut v = body.clone();
    let obj = v.as_object_mut()?;
    if obj.get("stream").and_then(serde_json::Value::as_bool) != Some(true) {
        return None;
    }
    let opts = obj
        .entry("stream_options")
        .or_insert_with(|| serde_json::json!({}));
    if !opts.is_object() {
        *opts = serde_json::json!({});
    }
    let opts = opts.as_object_mut()?;
    if opts
        .get("include_usage")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        return None;
    }
    opts.insert("include_usage".to_string(), serde_json::Value::Bool(true));
    Some(v)
}

/// A reader that copies every byte it serves into a shared buffer.
/// This is what lets the proxy stream upstream bytes to the client in real
/// time while still owning a full copy for parsing afterwards.
///
/// `complete` flips when upstream reaches EOF. `tiny_http` swallows the
/// client-side write errors (`BrokenPipe`, `ConnectionReset`) inside
/// `respond`, so a client that hangs up mid-stream is invisible there; the
/// only reliable sign is that the copy stopped before upstream was drained.
struct Tee<R: Read> {
    inner: R,
    buf: Arc<Mutex<Vec<u8>>>,
    complete: Arc<AtomicBool>,
}

impl<R: Read> Read for Tee<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(out)?;
        if n == 0 {
            self.complete.store(true, Ordering::SeqCst);
        } else if let Ok(mut b) = self.buf.lock() {
            b.extend_from_slice(&out[..n]);
        }
        Ok(n)
    }
}

const FORWARDED_HEADERS: &[&str] = &[
    "authorization",
    "x-api-key",
    "anthropic-version",
    "anthropic-beta",
    "openai-beta",
    "openai-organization",
    "openai-project",
    "content-type",
    "accept",
    "user-agent",
];

/// Headers that must not be relayed verbatim through a proxy (RFC 9110
/// §7.6.1) plus framing headers the tee stream re-computes.
fn is_hop_by_hop(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "proxy-connection"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "content-encoding"
    )
}

fn handle(mut request: tiny_http::Request, cfg: &ProxyConfig, repo: &Repo) -> Result<()> {
    let url = request.url().to_string();
    let method = request.method().clone();
    let (upstream_base, upstream_path) = route(&url, cfg);
    let full_url = format!("{}{}", upstream_base, upstream_path);

    let mut body = Vec::new();
    request.as_reader().read_to_end(&mut body)?;

    // Request-side metadata (model, prompt, agent identity).
    let mut body_json: Option<serde_json::Value> = serde_json::from_slice(&body).ok();
    if let Some(patched) = body_json
        .as_ref()
        .and_then(|v| with_stream_usage(v, &upstream_path))
    {
        body = serde_json::to_vec(&patched)?;
        body_json = Some(patched);
    }
    let user_agent = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("user-agent"))
        .map(|h| h.value.as_str().to_string());

    // Forward upstream. No overall timeout: SSE streams can run for minutes.
    // Non-2xx still has a body the client needs to see (error details), so
    // status codes are never turned into errors here.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut req = ureq::http::Request::builder()
        .method(method.as_str())
        .uri(&full_url);
    for name in FORWARDED_HEADERS {
        if let Some(h) = request.headers().iter().find(|h| h.field.equiv(name)) {
            req = req.header(*name, h.value.as_str());
        }
    }
    let upstream = if method == Method::Get {
        req.body(())
            .map_err(|e| anyhow!("invalid upstream request: {}", e))
            .and_then(|r| agent.run(r).map_err(Into::into))
    } else {
        req.body(body.as_slice())
            .map_err(|e| anyhow!("invalid upstream request: {}", e))
            .and_then(|r| agent.run(r).map_err(Into::into))
    };
    let upstream = match upstream {
        Ok(r) => r,
        Err(e) => {
            let resp = Response::from_string(format!("causari proxy: upstream unreachable: {}", e))
                .with_status_code(502);
            let _ = request.respond(resp);
            return Ok(());
        }
    };

    let status = upstream.status().as_u16();
    let content_type = upstream
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    // Forward every end-to-end response header (Retry-After, request ids,
    // x-ratelimit-*, openai-*/anthropic-* metadata). Hop-by-hop and framing
    // headers are dropped: the tee re-frames the body, and ureq may already
    // have decoded the content encoding.
    let mut headers = Vec::new();
    for (name, value) in upstream.headers() {
        if is_hop_by_hop(name.as_str()) {
            continue;
        }
        if let Ok(h) = Header::from_bytes(name.as_str().as_bytes(), value.as_bytes()) {
            headers.push(h);
        }
    }
    if !headers.iter().any(|h| h.field.equiv("content-type")) {
        headers.push(
            Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                .map_err(|_| anyhow!("invalid content-type header"))?,
        );
    }

    // Tee-stream the response: client gets bytes live, we keep a copy.
    let captured = Arc::new(Mutex::new(Vec::new()));
    let complete = Arc::new(AtomicBool::new(false));
    let tee = Tee {
        inner: upstream.into_body().into_reader(),
        buf: Arc::clone(&captured),
        complete: Arc::clone(&complete),
    };
    let response = Response::new(StatusCode(status), headers, tee, None, None);
    // When the client hangs up (or upstream breaks) part way through, the
    // provider has billed the call anyway and the bytes seen so far are
    // still evidence: the exchange is recorded as truncated, not dropped.
    let respond_failed = request.respond(response).is_err();
    let truncated = respond_failed || !complete.load(Ordering::SeqCst);

    if !is_completion_request(&method, &upstream_path) || status >= 400 {
        return Ok(());
    }
    let bytes = captured
        .lock()
        .map_err(|_| anyhow!("capture buffer poisoned"))?
        .clone();
    let exchange = record_exchange(
        repo,
        cfg,
        Captured {
            request_body: &body,
            request_json: body_json.as_ref(),
            response_bytes: &bytes,
            content_type: &content_type,
            user_agent,
            truncated,
        },
    )?;
    print_exchange(&exchange);
    Ok(())
}

/// One completion as it crossed the wire, ready to be turned into an
/// `Exchange`. `response_bytes` is everything relayed to the client; when
/// `truncated`, that is a prefix of what upstream sent.
struct Captured<'a> {
    request_body: &'a [u8],
    request_json: Option<&'a serde_json::Value>,
    response_bytes: &'a [u8],
    content_type: &'a str,
    user_agent: Option<String>,
    truncated: bool,
}

/// Parse the captured completion, optionally seal it, and append it to
/// `exchanges.jsonl`.
fn record_exchange(repo: &Repo, cfg: &ProxyConfig, c: Captured<'_>) -> Result<Exchange> {
    let requested_model = c
        .request_json
        .and_then(|v| v.get("model"))
        .and_then(|m| m.as_str())
        .map(String::from);
    let prompt = c.request_json.and_then(extract_prompt);

    let parsed = if c.content_type.contains("event-stream") {
        parse_sse(&String::from_utf8_lossy(c.response_bytes))
    } else {
        serde_json::from_slice::<serde_json::Value>(c.response_bytes)
            .map(|v| parse_response_json(&v))
            .unwrap_or_default()
    };
    let ParsedResponse {
        text,
        tokens_in,
        tokens_out,
        model: served_model,
    } = parsed;
    // The response names the model that actually answered (a dated snapshot
    // behind an alias, a fallback): that is what was billed.
    let model = served_model.or(requested_model);
    let cost_usd = estimate_cost(model.as_deref(), tokens_in, tokens_out);

    // Optionally emit a Crovia Seal over the exact wire bytes: the request
    // as sent upstream, the response as returned to the client. The seal
    // commits to hashes only — content never leaves the machine. It is
    // emitted first so the exchange record can carry its id.
    let seal_id = if let Some(sealer) = &cfg.sealer {
        let mut issuer = sealer.lock().map_err(|_| anyhow!("seal issuer poisoned"))?;
        let seal = issuer.emit(
            SealSubject {
                input: c.request_body,
                output: c.response_bytes,
                modality: "text",
            },
            SealGenerator {
                id: model.as_deref().unwrap_or("unknown"),
                version: None,
                params: seal_params(c.request_json),
            },
        )?;
        seal["seal_id"].as_str().map(String::from)
    } else {
        None
    };

    let exchange = Exchange {
        id: Some(crate::capture::new_exchange_id()?),
        ts_ms: now_ms(),
        agent: c.user_agent,
        model,
        prompt,
        response_text: text,
        tokens_in,
        tokens_out,
        cost_usd,
        request_sha256: Some(crate::seal::sha256_hex(c.request_body)),
        response_sha256: Some(crate::seal::sha256_hex(c.response_bytes)),
        seal_id,
        truncated: c.truncated,
    };
    append_jsonl(&exchanges_path(repo), &exchange)?;
    Ok(exchange)
}

fn print_exchange(e: &Exchange) {
    let prompt_preview = e
        .prompt
        .as_deref()
        .map(|p| {
            let first = p.lines().next().unwrap_or("");
            let mut s: String = first.chars().take(60).collect();
            if first.chars().count() > 60 {
                s.push('…');
            }
            s
        })
        .unwrap_or_else(|| "(no prompt)".to_string());
    println!(
        "  {} {}  {}{}  {}{}{}",
        "•".green(),
        e.model.as_deref().unwrap_or("unknown-model").cyan(),
        format_tokens(e.tokens_in, e.tokens_out).bright_black(),
        e.cost_usd
            .map(|c| format!("  ${:.4}", c))
            .unwrap_or_default()
            .bright_black(),
        format!("\"{}\"", prompt_preview).italic(),
        e.seal_id
            .as_deref()
            .map(|id| format!("  🔏 {}", id))
            .unwrap_or_default()
            .bright_black(),
        if e.truncated {
            "  (client disconnected; partial)".yellow()
        } else {
            "".normal()
        }
    );
}

fn format_tokens(tin: Option<u64>, tout: Option<u64>) -> String {
    match (tin, tout) {
        (Some(i), Some(o)) => format!("{}→{} tok", i, o),
        (Some(i), None) => format!("{} tok in", i),
        (None, Some(o)) => format!("{} tok out", o),
        (None, None) => "tokens n/a".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_requests_are_posts_to_completion_endpoints() {
        let post = Method::Post;
        assert!(is_completion_request(&post, "/v1/chat/completions"));
        assert!(is_completion_request(&post, "/v1/messages"));
        assert!(is_completion_request(&post, "/v1/responses"));
        assert!(is_completion_request(&post, "/v1/responses/"));
        assert!(is_completion_request(&post, "/v1/messages?beta=true"));
        assert!(is_completion_request(
            &post,
            "/openai/deployments/gpt-4o/chat/completions?api-version=2024-10-21"
        ));
    }

    #[test]
    fn side_endpoints_and_reads_are_not_completions() {
        let post = Method::Post;
        assert!(!is_completion_request(&post, "/v1/messages/count_tokens"));
        assert!(!is_completion_request(&post, "/v1/messages/batches"));
        assert!(!is_completion_request(
            &post,
            "/v1/responses/resp_123/cancel"
        ));
        assert!(!is_completion_request(&post, "/v1/embeddings"));
        assert!(!is_completion_request(
            &Method::Get,
            "/v1/responses/resp_123"
        ));
        assert!(!is_completion_request(&Method::Get, "/v1/responses"));
        assert!(!is_completion_request(
            &Method::Delete,
            "/v1/responses/resp_123"
        ));
    }

    #[test]
    fn streaming_chat_requests_get_usage_requested() {
        use serde_json::json;
        let path = "/v1/chat/completions";
        let body = json!({"model": "gpt-4o", "stream": true, "messages": []});
        let patched = with_stream_usage(&body, path).expect("patched");
        assert_eq!(patched["stream_options"]["include_usage"], json!(true));
        assert_eq!(patched["model"], json!("gpt-4o"), "rest of the body intact");

        // Existing stream_options are extended, not replaced.
        let body = json!({"stream": true, "stream_options": {"other": 1}});
        let patched = with_stream_usage(&body, path).unwrap();
        assert_eq!(patched["stream_options"]["other"], json!(1));
        assert_eq!(patched["stream_options"]["include_usage"], json!(true));

        // A malformed stream_options is replaced rather than forwarded broken.
        let body = json!({"stream": true, "stream_options": null});
        assert_eq!(
            with_stream_usage(&body, path).unwrap()["stream_options"]["include_usage"],
            json!(true)
        );
    }

    #[test]
    fn non_streaming_other_endpoints_and_explicit_opt_in_are_left_alone() {
        use serde_json::json;
        let path = "/v1/chat/completions";
        assert!(with_stream_usage(&json!({"model": "gpt-4o", "stream": false}), path).is_none());
        assert!(with_stream_usage(&json!({"model": "gpt-4o"}), path).is_none());
        assert!(
            with_stream_usage(
                &json!({"stream": true, "stream_options": {"include_usage": true}}),
                path
            )
            .is_none()
        );
        assert!(with_stream_usage(&json!({"stream": true}), "/v1/responses").is_none());
        assert!(with_stream_usage(&json!({"stream": true}), "/v1/messages").is_none());
        assert!(with_stream_usage(&json!([1, 2]), path).is_none());
    }

    #[test]
    fn tee_reports_completion_only_at_upstream_eof() {
        let upstream: &[u8] = b"data: one\n\ndata: two\n\n";
        let buf = Arc::new(Mutex::new(Vec::new()));
        let complete = Arc::new(AtomicBool::new(false));
        let mut tee = Tee {
            inner: upstream,
            buf: Arc::clone(&buf),
            complete: Arc::clone(&complete),
        };
        // The client-side copy stops after the first read: what a hung-up
        // client looks like from here.
        let mut out = [0u8; 11];
        tee.read_exact(&mut out).unwrap();
        assert_eq!(&out[..], b"data: one\n\n");
        assert!(!complete.load(Ordering::SeqCst));
        assert_eq!(buf.lock().unwrap().as_slice(), b"data: one\n\n");

        // Draining upstream flips the flag and the copy is whole.
        let mut rest = Vec::new();
        tee.read_to_end(&mut rest).unwrap();
        assert!(complete.load(Ordering::SeqCst));
        assert_eq!(buf.lock().unwrap().as_slice(), upstream);
    }

    #[test]
    fn a_stream_cut_by_the_client_is_still_recorded_as_truncated() {
        use serde_json::json;
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        let cfg = ProxyConfig {
            openai: String::new(),
            anthropic: String::new(),
            sealer: None,
        };
        let request = json!({"model": "gpt-4o", "stream": true,
            "messages": [{"role": "user", "content": "add the helper"}]});
        let request_body = serde_json::to_vec(&request).unwrap();
        // The client hung up after two chunks: no finish_reason, no usage.
        let partial = "data: {\"model\":\"gpt-4o-2024-08-06\",\"choices\":[{\"delta\":{\"content\":\"def helper():\\n\"}}]}\n\n\
                       data: {\"model\":\"gpt-4o-2024-08-06\",\"choices\":[{\"delta\":{\"content\":\"    return 4\"}}]}\n\n\
                       data: {\"model\":\"gpt-4o-2024-08-06\",\"choi";

        let e = record_exchange(
            &repo,
            &cfg,
            Captured {
                request_body: &request_body,
                request_json: Some(&request),
                response_bytes: partial.as_bytes(),
                content_type: "text/event-stream",
                user_agent: Some("aider/0.86".into()),
                truncated: true,
            },
        )
        .unwrap();
        assert!(e.truncated);
        assert_eq!(e.response_text, "def helper():\n    return 4");
        assert_eq!(e.model.as_deref(), Some("gpt-4o-2024-08-06"));
        assert_eq!(e.prompt.as_deref(), Some("add the helper"));
        assert_eq!((e.tokens_in, e.tokens_out, e.cost_usd), (None, None, None));

        // Persisted with the flag, and loadable by the join.
        let raw = std::fs::read_to_string(exchanges_path(&repo)).unwrap();
        assert!(raw.contains("\"truncated\":true"), "{raw}");
        let loaded = crate::capture::load_exchanges_since(&repo, 0).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded[0].truncated);

        // A complete exchange carries no flag at all, so older readers and
        // legacy lines (no field) mean the same thing: not truncated.
        let full = record_exchange(
            &repo,
            &cfg,
            Captured {
                request_body: &request_body,
                request_json: Some(&request),
                response_bytes: br#"{"model":"gpt-4o-2024-08-06","choices":[{"message":{"content":"ok"}}],"usage":{"prompt_tokens":3,"completion_tokens":1}}"#,
                content_type: "application/json",
                user_agent: None,
                truncated: false,
            },
        )
        .unwrap();
        assert!(!full.truncated);
        let last = std::fs::read_to_string(exchanges_path(&repo)).unwrap();
        assert!(!last.lines().last().unwrap().contains("truncated"));
    }
}
