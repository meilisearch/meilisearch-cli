use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_msc"))
}

fn unique_index() -> String {
    format!("test_{}", uuid::Uuid::new_v4().to_string().replace('-', ""))
}

#[test]
fn test_health() {
    let output = cli().args(["health"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("available"));
}

#[test]
fn test_version() {
    let output = cli().args(["version"]).output().unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("msc"));
    assert!(stderr.contains("meilisearch-cli"));
    assert!(stderr.contains("meilisearch server"));
}

#[test]
fn test_stats() {
    let output = cli().args(["stats"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("databaseSize"));
}

#[test]
fn test_index_lifecycle() {
    let idx = unique_index();

    // Create
    let output = cli()
        .args(["index", "create", &idx, "--primary-key", "id"])
        .output()
        .unwrap();
    assert!(output.status.success(), "create failed: {:?}", output);

    // Wait for creation task
    let stdout = String::from_utf8_lossy(&output.stdout);
    let task: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_uid = task["taskUid"].as_u64().unwrap();

    let _ = cli()
        .args(["task", "wait", &task_uid.to_string()])
        .output()
        .unwrap();

    // List
    let output = cli().args(["index", "list"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&idx));

    // Get
    let output = cli().args(["index", "get", &idx]).output().unwrap();
    assert!(output.status.success());

    // Stats
    let output = cli().args(["index", "stats", &idx]).output().unwrap();
    assert!(output.status.success());

    // Delete
    let output = cli().args(["index", "delete", &idx]).output().unwrap();
    assert!(output.status.success());
}

#[test]
fn test_document_crud() {
    let idx = unique_index();

    // Create index
    let output = cli()
        .args(["index", "create", &idx, "--primary-key", "id"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let task: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_uid = task["taskUid"].as_u64().unwrap();
    let _ = cli()
        .args(["task", "wait", &task_uid.to_string()])
        .output()
        .unwrap();

    // Add documents via stdin
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_msc"))
        .args(["document", "add", &idx])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    use std::io::Write;
    let stdin = child.stdin.as_mut().unwrap();
    stdin
        .write_all(b"[{\"id\": 1, \"title\": \"hello\"}, {\"id\": 2, \"title\": \"world\"}]")
        .unwrap();
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());

    // Wait for task
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Ok(task) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(task_uid) = task["taskUid"].as_u64() {
            let _ = cli()
                .args(["task", "wait", &task_uid.to_string()])
                .output()
                .unwrap();
        }
    }

    // List documents
    let output = cli().args(["document", "list", &idx]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello"));

    // Get single document
    let output = cli().args(["document", "get", &idx, "1"]).output().unwrap();
    assert!(output.status.success());

    // Delete single document
    let output = cli()
        .args(["document", "delete", &idx, "1"])
        .output()
        .unwrap();
    assert!(output.status.success());

    // Cleanup
    let _ = cli().args(["index", "delete", &idx]).output();
}

#[test]
fn test_search() {
    let idx = unique_index();

    // Create and populate index
    let output = cli()
        .args(["index", "create", &idx, "--primary-key", "id"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let task: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_uid = task["taskUid"].as_u64().unwrap();
    let _ = cli()
        .args(["task", "wait", &task_uid.to_string()])
        .output()
        .unwrap();

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_msc"))
        .args(["document", "add", &idx])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    use std::io::Write;
    let stdin = child.stdin.as_mut().unwrap();
    stdin
        .write_all(b"[{\"id\": 1, \"title\": \"The Great Gatsby\"}, {\"id\": 2, \"title\": \"Moby Dick\"}]")
        .unwrap();
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Ok(task) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(task_uid) = task["taskUid"].as_u64() {
            let _ = cli()
                .args(["task", "wait", &task_uid.to_string()])
                .output()
                .unwrap();
        }
    }

    // Search
    let output = cli().args(["search", &idx, "gatsby"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Gatsby"));

    // Cleanup
    let _ = cli().args(["index", "delete", &idx]).output();
}

#[test]
fn test_settings() {
    let idx = unique_index();

    // Create index
    let output = cli().args(["index", "create", &idx]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let task: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_uid = task["taskUid"].as_u64().unwrap();
    let _ = cli()
        .args(["task", "wait", &task_uid.to_string()])
        .output()
        .unwrap();

    // Get settings
    let output = cli().args(["settings", "get", &idx]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rankingRules"));

    // Cleanup
    let _ = cli().args(["index", "delete", &idx]).output();
}

#[test]
fn test_task_list() {
    let output = cli()
        .args(["task", "list", "--limit", "5"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("results"));
}

#[test]
fn test_project_commands() {
    // List projects
    let output = cli().args(["project", "list"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("local"));

    // Current project
    let output = cli().args(["project", "current"]).output().unwrap();
    assert!(output.status.success());
}

#[test]
fn test_raw_output() {
    let output = cli().args(["--raw", "health"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Raw output should be compact JSON
    let parsed: serde_json::Value = serde_json::from_str(&stdout.trim()).unwrap();
    assert_eq!(parsed["status"], "available");
}

#[test]
fn test_import_json_file() {
    let idx = unique_index();

    // Create index
    let output = cli()
        .args(["index", "create", &idx, "--primary-key", "id"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let task: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_uid = task["taskUid"].as_u64().unwrap();
    let _ = cli()
        .args(["task", "wait", &task_uid.to_string()])
        .output()
        .unwrap();

    // Create temp file
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        tmp.path(),
        r#"[{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}]"#,
    )
    .unwrap();

    // Import
    let output = cli()
        .args(["import", &idx, "--file", tmp.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "import failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify
    let output = cli().args(["document", "list", &idx]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Alice"));

    // Cleanup
    let _ = cli().args(["index", "delete", &idx]).output();
}
