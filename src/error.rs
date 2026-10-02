//! Typed CLI errors with stable exit codes and a machine-readable JSON shape.
//!
//! Commands keep returning `anyhow::Result`; when a failure needs a specific
//! exit code it carries a [`CliError`] somewhere in its anyhow chain, and
//! [`report`] turns any error into an [`ErrorReport`] for printing.

use std::fmt;

use serde_json::{Value, json};

/// Category of failure. Each kind maps to a stable process exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Anything not covered below.
    General,
    /// Invalid arguments, bad input, missing configuration, non-interactive misuse.
    Usage,
    /// Missing or invalid API key (HTTP 401/403).
    Auth,
    /// The resource does not exist (HTTP 404).
    NotFound,
    /// Any other error returned by the Meilisearch API.
    Api,
    /// The server could not be reached.
    Network,
    /// An asynchronous task finished with status `failed` or `canceled`.
    TaskFailed,
    /// Waiting for a task exceeded the timeout.
    Timeout,
}

impl ErrorKind {
    pub const ALL: [ErrorKind; 8] = [
        ErrorKind::General,
        ErrorKind::Usage,
        ErrorKind::Auth,
        ErrorKind::NotFound,
        ErrorKind::Api,
        ErrorKind::Network,
        ErrorKind::TaskFailed,
        ErrorKind::Timeout,
    ];

    pub fn exit_code(self) -> u8 {
        match self {
            ErrorKind::General => 1,
            ErrorKind::Usage => 2,
            ErrorKind::Auth => 3,
            ErrorKind::NotFound => 4,
            ErrorKind::Api => 5,
            ErrorKind::Network => 6,
            ErrorKind::TaskFailed => 7,
            ErrorKind::Timeout => 8,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ErrorKind::General => "general",
            ErrorKind::Usage => "usage",
            ErrorKind::Auth => "auth",
            ErrorKind::NotFound => "not_found",
            ErrorKind::Api => "api",
            ErrorKind::Network => "network",
            ErrorKind::TaskFailed => "task_failed",
            ErrorKind::Timeout => "timeout",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ErrorKind::General => "Unclassified error",
            ErrorKind::Usage => "Invalid arguments, input, or configuration",
            ErrorKind::Auth => "Missing or invalid API key (HTTP 401/403)",
            ErrorKind::NotFound => "Resource not found (HTTP 404)",
            ErrorKind::Api => "Other Meilisearch API error",
            ErrorKind::Network => "Meilisearch server unreachable",
            ErrorKind::TaskFailed => "Task finished as failed or canceled",
            ErrorKind::Timeout => "Timed out waiting for a task",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CliError {
    pub kind: ErrorKind,
    pub code: String,
    pub message: String,
    pub error_type: Option<String>,
    pub link: Option<String>,
    pub http_status: Option<u16>,
    pub hint: Option<String>,
    /// Final task object, for `TaskFailed` errors.
    pub task: Option<Value>,
}

impl CliError {
    pub fn new(kind: ErrorKind, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind,
            code: code.into(),
            message: message.into(),
            error_type: None,
            link: None,
            http_status: None,
            hint: None,
            task: None,
        }
    }

    pub fn usage(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Usage, code, message)
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Build an error from a non-2xx Meilisearch response body.
    pub fn from_api(status: u16, body: &str) -> Self {
        let json: Option<Value> = serde_json::from_str(body).ok();
        let field = |k: &str| {
            json.as_ref()
                .and_then(|j| j.get(k))
                .and_then(Value::as_str)
                .map(String::from)
        };
        let code = field("code").unwrap_or_else(|| format!("http_{status}"));
        let message = field("message").unwrap_or_else(|| {
            let body = body.trim();
            if body.is_empty() {
                format!("HTTP {status} (empty response)")
            } else {
                format!("HTTP {status}: {}", truncate(body, 500))
            }
        });
        let kind = classify_api(status, &code);
        let hint = match kind {
            ErrorKind::Auth if code == "missing_master_key" => Some(
                "This endpoint only exists when the server is started with a master key".to_string(),
            ),
            ErrorKind::Auth => Some(
                "Pass an API key with --api-key or MSC_API_KEY, or configure one with `msc project update`"
                    .to_string(),
            ),
            _ => None,
        };
        Self {
            kind,
            code,
            message,
            error_type: field("type"),
            link: field("link"),
            http_status: Some(status),
            hint,
            task: None,
        }
    }

    /// Build an error for a task that finished as `failed` or `canceled`.
    pub fn task_failed(task: Value) -> Self {
        let uid = task["uid"].as_u64().unwrap_or_default();
        let status = task["status"].as_str().unwrap_or("failed").to_string();
        let err = &task["error"];
        let (code, message) = if status == "canceled" {
            (
                "task_canceled".to_string(),
                format!("Task {uid} was canceled"),
            )
        } else {
            (
                err["code"].as_str().unwrap_or("task_failed").to_string(),
                format!(
                    "Task {uid} failed: {}",
                    err["message"].as_str().unwrap_or("unknown error")
                ),
            )
        };
        Self {
            kind: ErrorKind::TaskFailed,
            code,
            message,
            error_type: err["type"].as_str().map(String::from),
            link: err["link"].as_str().map(String::from),
            http_status: None,
            hint: None,
            task: Some(task),
        }
    }

    pub fn task_timeout(task_uid: u64, timeout_ms: u64) -> Self {
        Self::new(
            ErrorKind::Timeout,
            "task_timeout",
            format!("Task {task_uid} did not finish within {timeout_ms}ms"),
        )
        .with_hint(format!(
            "The task is still running. Check it later with `msc task wait {task_uid}`"
        ))
    }

    pub fn network(url: &str, source: &reqwest::Error) -> Self {
        let code = if source.is_timeout() {
            "request_timeout"
        } else {
            "connection_failed"
        };
        Self::new(
            ErrorKind::Network,
            code,
            format!("Could not reach Meilisearch at {url}: {}", root_cause(source)),
        )
        .with_hint(
            "Is the server running? Start one with `msc local start`, or target another with --url / --project",
        )
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

fn classify_api(status: u16, code: &str) -> ErrorKind {
    match (status, code) {
        (401 | 403, _) | (_, "missing_authorization_header" | "invalid_api_key") => ErrorKind::Auth,
        (404, _) => ErrorKind::NotFound,
        (_, c) if c.ends_with("_not_found") => ErrorKind::NotFound,
        _ => ErrorKind::Api,
    }
}

fn root_cause(err: &dyn std::error::Error) -> String {
    let mut cur = err;
    while let Some(next) = cur.source() {
        cur = next;
    }
    cur.to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

/// Everything needed to print an error and pick an exit code.
#[derive(Debug, Clone)]
pub struct ErrorReport {
    pub error: CliError,
    /// Full human-readable message including anyhow context.
    pub display: String,
}

impl ErrorReport {
    pub fn exit_code(&self) -> u8 {
        self.error.kind.exit_code()
    }

    pub fn to_json(&self) -> Value {
        let e = &self.error;
        let mut obj = json!({
            "kind": e.kind.as_str(),
            "code": e.code,
            "message": self.display,
            "exitCode": e.kind.exit_code(),
        });
        if let Some(t) = &e.error_type {
            obj["type"] = json!(t);
        }
        if let Some(l) = &e.link {
            obj["link"] = json!(l);
        }
        if let Some(s) = e.http_status {
            obj["httpStatus"] = json!(s);
        }
        if let Some(h) = &e.hint {
            obj["hint"] = json!(h);
        }
        // The full task is printed on stdout; reference it here.
        if let Some(uid) = e.task.as_ref().and_then(|t| t["uid"].as_u64()) {
            obj["taskUid"] = json!(uid);
        }
        json!({ "error": obj })
    }

    pub fn to_human(&self) -> String {
        let mut out = format!("Error: {}", self.display);
        if let Some(h) = &self.error.hint {
            out.push_str(&format!("\n  hint: {h}"));
        }
        if let Some(l) = &self.error.link {
            out.push_str(&format!("\n  docs: {l}"));
        }
        out
    }
}

/// Classify any error into a report, looking through the anyhow chain.
pub fn report(err: &anyhow::Error) -> ErrorReport {
    let display = format!("{err:#}");
    for cause in err.chain() {
        if let Some(e) = cause.downcast_ref::<CliError>() {
            return ErrorReport {
                error: e.clone(),
                display,
            };
        }
    }
    for cause in err.chain() {
        if let Some(e) = cause.downcast_ref::<reqwest::Error>()
            && (e.is_connect() || e.is_timeout() || e.is_request())
        {
            let url = e.url().map(|u| u.to_string()).unwrap_or_default();
            let error = CliError::network(&url, e);
            return ErrorReport {
                display: error.message.clone(),
                error,
            };
        }
    }
    ErrorReport {
        error: CliError::new(ErrorKind::General, "error", display.clone()),
        display,
    }
}

/// True when the error is (or wraps) a not-found error.
pub fn is_not_found(err: &anyhow::Error) -> bool {
    report(err).error.kind == ErrorKind::NotFound
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_errors_are_classified() {
        let body = r#"{"message":"Index `x` not found.","code":"index_not_found","type":"invalid_request","link":"https://docs.meilisearch.com/errors#index_not_found"}"#;
        let e = CliError::from_api(404, body);
        assert_eq!(e.kind, ErrorKind::NotFound);
        assert_eq!(e.code, "index_not_found");
        assert_eq!(e.error_type.as_deref(), Some("invalid_request"));
        assert!(e.link.is_some());

        let e = CliError::from_api(
            401,
            r#"{"message":"m","code":"missing_authorization_header"}"#,
        );
        assert_eq!(e.kind, ErrorKind::Auth);
        assert_eq!(e.kind.exit_code(), 3);

        let e = CliError::from_api(400, r#"{"message":"bad","code":"invalid_search_q"}"#);
        assert_eq!(e.kind, ErrorKind::Api);

        let e = CliError::from_api(502, "<html>Bad Gateway</html>");
        assert_eq!(e.code, "http_502");
        assert!(e.message.contains("Bad Gateway"));
    }

    #[test]
    fn task_failure_carries_task() {
        let task = json!({"uid": 7, "status": "failed", "error": {"code": "index_already_exists", "message": "exists"}});
        let e = CliError::task_failed(task);
        assert_eq!(e.kind.exit_code(), 7);
        assert_eq!(e.code, "index_already_exists");
        assert!(e.task.is_some());
        let r = ErrorReport {
            display: e.message.clone(),
            error: e,
        };
        assert_eq!(r.to_json()["error"]["taskUid"], 7);
    }

    #[test]
    fn report_finds_cli_error_through_context() {
        let err = anyhow::Error::new(CliError::usage("bad_input", "nope")).context("while doing x");
        let r = report(&err);
        assert_eq!(r.exit_code(), 2);
        assert_eq!(r.to_json()["error"]["code"], "bad_input");
        assert!(r.display.contains("while doing x"));
    }

    #[test]
    fn unknown_errors_are_general() {
        let r = report(&anyhow::anyhow!("boom"));
        assert_eq!(r.exit_code(), 1);
        assert_eq!(r.to_json()["error"]["kind"], "general");
    }

    #[test]
    fn exit_codes_are_unique() {
        let mut codes: Vec<u8> = ErrorKind::ALL.iter().map(|k| k.exit_code()).collect();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), ErrorKind::ALL.len());
    }
}
