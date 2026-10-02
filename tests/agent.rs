//! Agent-facing contract: JSON output, exit codes, --wait, idempotency,
//! dry runs, --select, schema and the MCP server.

use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

fn test_url() -> String {
    std::env::var("MSC_TEST_URL").unwrap_or_else(|_| "http://localhost:7700".to_string())
}

fn config_path(name: &str) -> String {
    format!("{}/{name}.toml", env!("CARGO_TARGET_TMPDIR"))
}

fn cli() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_msc"));
    cmd.env("MSC_URL", test_url())
        .env("MSC_CONFIG", config_path("agent-config"))
        .env_remove("MSC_API_KEY")
        .env_remove("MSC_PROJECT")
        .env_remove("MSC_WAIT")
        .stdin(Stdio::null());
    cmd
}

fn run(args: &[&str]) -> Output {
    cli().args(args).output().unwrap()
}

fn run_stdin(args: &[&str], input: &[u8]) -> Output {
    let mut child = cli()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn stdout_json(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}): {}",
            String::from_utf8_lossy(&o.stdout)
        )
    })
}

fn stderr_error(o: &Output) -> Value {
    let v: Value = serde_json::from_slice(&o.stderr).unwrap_or_else(|e| {
        panic!(
            "stderr is not JSON ({e}): {}",
            String::from_utf8_lossy(&o.stderr)
        )
    });
    v["error"].clone()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn unique_index() -> String {
    format!("agent_{}", uuid::Uuid::new_v4().simple())
}

/// Creates an index (waiting for it) and deletes it on drop.
struct TempIndex(String);

impl TempIndex {
    fn new() -> Self {
        let uid = unique_index();
        let o = run(&["index", "create", &uid, "--primary-key", "id", "--wait"]);
        assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
        TempIndex(uid)
    }
}

impl Drop for TempIndex {
    fn drop(&mut self) {
        let _ = run(&["index", "delete", &self.0]);
    }
}

#[test]
fn piped_output_is_compact_json() {
    let o = run(&["health"]);
    assert_eq!(code(&o), 0);
    let text = String::from_utf8_lossy(&o.stdout);
    assert_eq!(text.trim(), r#"{"status":"available"}"#);
}

#[test]
fn pretty_flag_forces_human_output() {
    let o = run(&["health", "--pretty"]);
    assert!(String::from_utf8_lossy(&o.stdout).contains("\n  \"status\""));
}

#[test]
fn not_found_exits_4_with_json_error() {
    let o = run(&["index", "get", &unique_index()]);
    assert_eq!(code(&o), 4);
    assert!(o.stdout.is_empty());
    let err = stderr_error(&o);
    assert_eq!(err["kind"], "not_found");
    assert_eq!(err["code"], "index_not_found");
    assert_eq!(err["httpStatus"], 404);
    assert!(err["link"].is_string());
}

#[test]
fn unreachable_server_exits_6() {
    let o = run(&["health", "--url", "http://127.0.0.1:9"]);
    assert_eq!(code(&o), 6);
    let err = stderr_error(&o);
    assert_eq!(err["kind"], "network");
    assert!(err["hint"].is_string());
}

#[test]
fn usage_errors_exit_2_with_json_error() {
    let o = run(&["index", "create"]);
    assert_eq!(code(&o), 2);
    assert_eq!(stderr_error(&o)["code"], "missing_argument");

    let o = run(&["settings", "get", "x", "not-a-setting"]);
    assert_eq!(code(&o), 2);
    assert_eq!(stderr_error(&o)["code"], "invalid_settings_sub_resource");
}

#[test]
fn interactive_commands_refuse_without_terminal() {
    for args in [
        &["settings", "edit", "x"][..],
        &["search", "x", "-i"][..],
        &["chat", "-i"][..],
    ] {
        let o = run(args);
        assert_eq!(code(&o), 2, "{args:?}");
        assert_eq!(stderr_error(&o)["code"], "non_interactive", "{args:?}");
    }
}

#[test]
fn wait_returns_final_task_and_failures_exit_7() {
    let idx = TempIndex::new();

    // Creating it again fails asynchronously.
    let o = run(&["index", "create", &idx.0, "--wait"]);
    assert_eq!(code(&o), 7);
    let task = stdout_json(&o);
    assert_eq!(task["status"], "failed");
    let err = stderr_error(&o);
    assert_eq!(err["kind"], "task_failed");
    assert_eq!(err["code"], "index_already_exists");
    assert_eq!(err["taskUid"], task["uid"]);

    // `task wait` reports the same failure.
    let o = run(&["task", "wait", &task["uid"].to_string()]);
    assert_eq!(code(&o), 7);
}

#[test]
fn idempotent_create_and_delete() {
    let idx = TempIndex::new();
    let o = run(&["index", "create", &idx.0, "--if-not-exists"]);
    assert_eq!(code(&o), 0);
    let v = stdout_json(&o);
    assert_eq!(v["skipped"], true);
    assert_eq!(v["reason"], "already_exists");

    let missing = unique_index();
    let o = run(&["index", "delete", &missing, "--if-exists"]);
    assert_eq!(code(&o), 0);
    assert_eq!(stdout_json(&o)["reason"], "not_found");
}

#[test]
fn documents_wait_select_and_dry_run() {
    let idx = TempIndex::new();
    let o = run_stdin(
        &["document", "add", &idx.0, "--format", "ndjson", "--wait"],
        b"{\"id\":1,\"title\":\"Dune\",\"year\":1965}\n{\"id\":2,\"title\":\"Emma\",\"year\":1815}\n",
    );
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let task = stdout_json(&o);
    assert_eq!(task["status"], "succeeded");
    assert_eq!(task["details"]["indexedDocuments"], 2);

    let o = run(&["search", &idx.0, "dune", "--select", "id"]);
    let v = stdout_json(&o);
    assert_eq!(v["hits"], json!([{"id": 1}]));
    assert!(v["estimatedTotalHits"].is_number());

    let o = run(&["document", "delete-all", &idx.0, "--dry-run"]);
    let v = stdout_json(&o);
    assert_eq!(v["dryRun"], true);
    assert_eq!(v["matchedDocuments"], 2);
    // Nothing was deleted.
    let o = run(&["index", "stats", &idx.0, "--select", "numberOfDocuments"]);
    assert_eq!(stdout_json(&o)["numberOfDocuments"], 2);
}

#[test]
fn settings_dry_run_shows_changes_only() {
    let idx = TempIndex::new();
    let o = run_stdin(
        &["settings", "update", &idx.0, "--dry-run"],
        br#"{"searchableAttributes": ["title"], "stopWords": []}"#,
    );
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let v = stdout_json(&o);
    assert_eq!(
        v["changes"],
        json!({"searchableAttributes": {"from": ["*"], "to": ["title"]}})
    );
    let o = run(&["settings", "get", &idx.0, "searchable-attributes"]);
    assert_eq!(stdout_json(&o), json!(["*"]));
}

#[test]
fn import_returns_summary_and_events() {
    let idx = TempIndex::new();
    let o = run_stdin(&["import", &idx.0], br#"[{"id":1},{"id":2},{"id":3}]"#);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let v = stdout_json(&o);
    assert_eq!(v["status"], "succeeded");
    assert_eq!(v["indexedDocuments"], 3);
    assert!(o.stderr.is_empty(), "no status text in JSON mode");

    let o = run_stdin(
        &["import", &idx.0, "--events", "--batch-size", "12"],
        br#"[{"id":4},{"id":5}]"#,
    );
    let events: Vec<Value> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(events.first().unwrap()["event"], "batch_enqueued");
    let done = events.last().unwrap();
    assert_eq!(done["event"], "done");
    assert_eq!(done["summary"]["indexedDocuments"], 2);
}

#[test]
fn project_commands_are_non_interactive() {
    let cfg = config_path(&format!("projects-{}", uuid::Uuid::new_v4().simple()));
    let p = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_msc"))
            .args(args)
            .env("MSC_CONFIG", &cfg)
            .env_remove("MSC_URL")
            .env_remove("MSC_API_KEY")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let o = p(&["project", "add", "staging"]);
    assert_eq!(code(&o), 2);
    assert_eq!(stderr_error(&o)["code"], "non_interactive");

    let o = p(&[
        "project",
        "add",
        "staging",
        "--url",
        "http://s:7700",
        "--api-key",
        "k",
    ]);
    assert_eq!(code(&o), 0);
    let v = stdout_json(&o);
    assert_eq!(v["apiKeySet"], true);
    assert!(v.get("apiKey").is_none(), "never print keys");

    assert_eq!(
        code(&p(&["project", "add", "staging", "--url", "http://x"])),
        2
    );
    assert_eq!(
        code(&p(&["project", "add", "staging", "--if-not-exists"])),
        0
    );

    let o = p(&["project", "update", "staging", "--remove-api-key"]);
    assert_eq!(stdout_json(&o)["apiKeySet"], false);

    let o = p(&["project", "list", "--select", "name"]);
    let names: Vec<Value> = stdout_json(&o)["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].clone())
        .collect();
    assert!(names.contains(&json!("staging")));

    assert_eq!(code(&p(&["project", "remove", "nope", "--if-exists"])), 0);
    let _ = std::fs::remove_file(&cfg);
}

#[test]
fn schema_lists_commands_and_exit_codes() {
    let o = run(&["schema"]);
    let v = stdout_json(&o);
    assert_eq!(
        v["exitCodes"]["7"].as_str().unwrap().split(':').next(),
        Some("task_failed")
    );
    assert!(
        v["globalArgs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["name"] == "wait")
    );
    let o = run(&["schema", "index", "delete"]);
    let v = stdout_json(&o);
    let names: Vec<&str> = v["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["uid", "if_exists", "dry_run"]);
    assert_eq!(code(&run(&["schema", "nope"])), 2);
}

#[test]
fn mcp_server_lists_and_calls_tools() {
    let idx = unique_index();
    let requests = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "index_create", "arguments": {"uid": idx, "primary_key": "id", "wait": true}}}),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "document_add", "arguments": {"uid": idx, "input": [{"id": 1, "title": "Dune"}], "wait": true}}}),
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "search", "arguments": {"uid": idx, "query": "dune", "select": ["title"]}}}),
        json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "index_get", "arguments": {"uid": unique_index()}}}),
        json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "index_delete", "arguments": {"uid": idx, "wait": true}}}),
        json!({"jsonrpc": "2.0", "id": 8, "method": "nope"}),
    ];
    let input: String = requests.iter().map(|r| format!("{r}\n")).collect();
    let o = run_stdin(&["mcp"], input.as_bytes());
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let responses: Vec<Value> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    // The notification gets no response.
    assert_eq!(responses.len(), requests.len() - 1);
    let by_id = |id: i64| responses.iter().find(|r| r["id"] == id).unwrap().clone();

    assert_eq!(by_id(1)["result"]["serverInfo"]["name"], "msc");
    let tools = by_id(2)["result"]["tools"].as_array().unwrap().clone();
    assert!(tools.iter().any(|t| t["name"] == "search"));
    assert!(!tools.iter().any(|t| t["name"] == "settings_edit"));

    let text = |id: i64| -> Value {
        serde_json::from_str(by_id(id)["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    };
    assert_eq!(text(3)["status"], "succeeded");
    assert_eq!(text(4)["status"], "succeeded");
    assert_eq!(text(5)["hits"], json!([{"title": "Dune"}]));
    assert_eq!(by_id(6)["result"]["isError"], true);
    assert_eq!(text(6)["error"]["kind"], "not_found");
    assert_eq!(text(7)["status"], "succeeded");
    assert_eq!(by_id(8)["error"]["code"], -32601);
}

#[test]
fn plugin_manifest_matches_crate() {
    let root = env!("CARGO_MANIFEST_DIR");
    let plugin: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{root}/.claude-plugin/plugin.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        plugin["version"],
        env!("CARGO_PKG_VERSION"),
        "bump plugin.json with Cargo.toml"
    );
    assert_eq!(plugin["mcpServers"]["meilisearch"]["args"], json!(["mcp"]));
    let market: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{root}/.claude-plugin/marketplace.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(market["plugins"][0]["name"], plugin["name"]);
}

#[test]
fn skill_install_and_uninstall() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_str().unwrap();
    let o = run(&["skill", "install", "--dir", d]);
    assert_eq!(code(&o), 0);
    assert_eq!(stdout_json(&o)["results"][0]["status"], "installed");
    let installed = std::fs::read_to_string(dir.path().join("msc/SKILL.md")).unwrap();
    assert_eq!(
        installed.as_bytes(),
        run(&["skill", "show"]).stdout.as_slice()
    );
    let o = run(&["skill", "uninstall", "--dir", d]);
    assert_eq!(stdout_json(&o)["results"][0]["removed"], true);
    assert!(!dir.path().join("msc").exists());
}

#[test]
fn api_escape_hatch_reaches_any_route() {
    let idx = TempIndex::new();
    let path = format!("/indexes/{}/documents", idx.0);
    let o = run_stdin(
        &[
            "api",
            "POST",
            &path,
            "--content-type",
            "application/x-ndjson",
            "--wait",
        ],
        b"{\"id\":1,\"title\":\"Dune\"}\n",
    );
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(stdout_json(&o)["status"], "succeeded");

    let o = run(&["api", "get", &path, "-p", "fields=id", "-p", "limit=1"]);
    assert_eq!(stdout_json(&o)["results"], json!([{"id": 1}]));

    let o = run(&["api", "GET", &format!("/indexes/{}", unique_index())]);
    assert_eq!(code(&o), 4);
    assert_eq!(code(&run(&["api", "GET", "indexes"])), 2);
    assert_eq!(code(&run(&["api", "POST", "/indexes", "-d", "{nope"])), 2);
}

#[test]
fn search_body_merges_extra_parameters() {
    let idx = TempIndex::new();
    let o = run_stdin(
        &["document", "add", &idx.0, "--wait"],
        br#"[{"id":1,"title":"Dune"}]"#,
    );
    assert_eq!(code(&o), 0);
    let o = run(&[
        "search",
        &idx.0,
        "dune",
        "--limit",
        "5",
        "--body",
        r#"{"showRankingScore":true,"limit":1}"#,
    ]);
    let v = stdout_json(&o);
    assert_eq!(v["limit"], 1, "--body overrides flags");
    assert!(v["hits"][0]["_rankingScore"].is_number());
    assert_eq!(code(&run(&["search", &idx.0, "x", "--body", "[1]"])), 2);
}

#[test]
fn whoami_reports_target_and_auth() {
    let o = run(&["whoami"]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let v = stdout_json(&o);
    assert_eq!(v["url"], test_url());
    assert_eq!(v["source"], "env MSC_URL");
    assert_eq!(v["reachable"], true);
    assert_eq!(v["auth"], "ok");

    let o = run(&["whoami", "--url", "http://127.0.0.1:9"]);
    assert_eq!(code(&o), 6);
    assert_eq!(stdout_json(&o)["reachable"], false);
}

#[test]
fn table_output_is_explicit_and_tabular() {
    let o = run(&["health", "--table"]);
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(text.starts_with("key     value"), "{text}");
    assert!(text.contains("status  available"));
    assert_eq!(code(&run(&["health", "--table", "--json"])), 2);
}
