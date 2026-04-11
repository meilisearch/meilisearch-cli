use anyhow::{Context, Result, bail};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct TaskFilters {
    pub uids: Option<String>,
    pub statuses: Option<String>,
    pub types: Option<String>,
    pub index_uids: Option<String>,
    pub canceled_by: Option<String>,
    pub before_enqueued_at: Option<String>,
    pub after_enqueued_at: Option<String>,
    pub before_started_at: Option<String>,
    pub after_started_at: Option<String>,
    pub before_finished_at: Option<String>,
    pub after_finished_at: Option<String>,
    pub from: Option<u64>,
    pub limit: Option<u64>,
}

impl TaskFilters {
    pub fn to_query_string(&self) -> String {
        let mut params = vec![];
        if let Some(v) = &self.uids {
            params.push(format!("uids={v}"));
        }
        if let Some(v) = &self.statuses {
            params.push(format!("statuses={v}"));
        }
        if let Some(v) = &self.types {
            params.push(format!("types={v}"));
        }
        if let Some(v) = &self.index_uids {
            params.push(format!("indexUids={v}"));
        }
        if let Some(v) = &self.canceled_by {
            params.push(format!("canceledBy={v}"));
        }
        if let Some(v) = &self.before_enqueued_at {
            params.push(format!("beforeEnqueuedAt={v}"));
        }
        if let Some(v) = &self.after_enqueued_at {
            params.push(format!("afterEnqueuedAt={v}"));
        }
        if let Some(v) = &self.before_started_at {
            params.push(format!("beforeStartedAt={v}"));
        }
        if let Some(v) = &self.after_started_at {
            params.push(format!("afterStartedAt={v}"));
        }
        if let Some(v) = &self.before_finished_at {
            params.push(format!("beforeFinishedAt={v}"));
        }
        if let Some(v) = &self.after_finished_at {
            params.push(format!("afterFinishedAt={v}"));
        }
        if let Some(v) = self.from {
            params.push(format!("from={v}"));
        }
        if let Some(v) = self.limit {
            params.push(format!("limit={v}"));
        }
        if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MeiliClient {
    pub base_url: String,
    http: reqwest::Client,
}

#[allow(dead_code)]
impl MeiliClient {
    pub fn new(url: &str, api_key: Option<&str>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        if let Some(key) = api_key {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", key))
                    .context("Invalid API key characters")?,
            );
        }
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;
        Ok(Self {
            base_url: url.trim_end_matches('/').to_string(),
            http,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    // ── Health / Version / Stats ──────────────────────────────────

    pub async fn health(&self) -> Result<Value> {
        self.get("/health").await
    }

    pub async fn version(&self) -> Result<Value> {
        self.get("/version").await
    }

    pub async fn stats(&self) -> Result<Value> {
        self.get("/stats").await
    }

    // ── Indexes ───────────────────────────────────────────────────

    pub async fn list_indexes(&self, offset: Option<u64>, limit: Option<u64>) -> Result<Value> {
        let mut params = vec![];
        if let Some(o) = offset {
            params.push(format!("offset={o}"));
        }
        if let Some(l) = limit {
            params.push(format!("limit={l}"));
        }
        let qs = if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        };
        self.get(&format!("/indexes{qs}")).await
    }

    pub async fn create_index(&self, uid: &str, primary_key: Option<&str>) -> Result<Value> {
        let mut body = serde_json::json!({ "uid": uid });
        if let Some(pk) = primary_key {
            body["primaryKey"] = serde_json::json!(pk);
        }
        self.post("/indexes", &body).await
    }

    pub async fn get_index(&self, uid: &str) -> Result<Value> {
        self.get(&format!("/indexes/{uid}")).await
    }

    pub async fn delete_index(&self, uid: &str) -> Result<Value> {
        self.delete(&format!("/indexes/{uid}")).await
    }

    pub async fn index_stats(&self, uid: &str) -> Result<Value> {
        self.get(&format!("/indexes/{uid}/stats")).await
    }

    pub async fn swap_indexes(&self, swaps: &[(&str, &str)]) -> Result<Value> {
        let body: Vec<Value> = swaps
            .iter()
            .map(|(a, b)| {
                serde_json::json!({
                    "indexes": [a, b]
                })
            })
            .collect();
        self.post("/swap-indexes", &serde_json::json!(body)).await
    }

    // ── Indexes (update) ────────────────────────────────────────────

    pub async fn update_index(&self, uid: &str, primary_key: Option<&str>) -> Result<Value> {
        let mut body = serde_json::json!({});
        if let Some(pk) = primary_key {
            body["primaryKey"] = serde_json::json!(pk);
        }
        self.patch(&format!("/indexes/{uid}"), &body).await
    }

    // ── Documents ─────────────────────────────────────────────────

    pub async fn get_documents(
        &self,
        uid: &str,
        offset: Option<u64>,
        limit: Option<u64>,
        fields: Option<&str>,
    ) -> Result<Value> {
        let mut params = vec![];
        if let Some(o) = offset {
            params.push(format!("offset={o}"));
        }
        if let Some(l) = limit {
            params.push(format!("limit={l}"));
        }
        if let Some(f) = fields {
            params.push(format!("fields={f}"));
        }
        let qs = if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        };
        self.get(&format!("/indexes/{uid}/documents{qs}")).await
    }

    pub async fn get_document(&self, uid: &str, doc_id: &str) -> Result<Value> {
        self.get(&format!("/indexes/{uid}/documents/{doc_id}"))
            .await
    }

    pub async fn add_documents(
        &self,
        uid: &str,
        documents: &Value,
        primary_key: Option<&str>,
    ) -> Result<Value> {
        let mut qs = String::new();
        if let Some(pk) = primary_key {
            qs = format!("?primaryKey={pk}");
        }
        self.post(&format!("/indexes/{uid}/documents{qs}"), documents)
            .await
    }

    pub async fn add_documents_raw(
        &self,
        uid: &str,
        body: Vec<u8>,
        content_type: &str,
        primary_key: Option<&str>,
    ) -> Result<Value> {
        let mut qs = String::new();
        if let Some(pk) = primary_key {
            qs = format!("?primaryKey={pk}");
        }
        let resp = self
            .http
            .post(self.url(&format!("/indexes/{uid}/documents{qs}")))
            .header(CONTENT_TYPE, content_type)
            .body(body)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn delete_document(&self, uid: &str, doc_id: &str) -> Result<Value> {
        self.delete(&format!("/indexes/{uid}/documents/{doc_id}"))
            .await
    }

    pub async fn delete_all_documents(&self, uid: &str) -> Result<Value> {
        self.delete(&format!("/indexes/{uid}/documents")).await
    }

    pub async fn delete_documents_batch(&self, uid: &str, ids: &[Value]) -> Result<Value> {
        self.post(
            &format!("/indexes/{uid}/documents/delete-batch"),
            &serde_json::json!(ids),
        )
        .await
    }

    pub async fn add_or_update_documents_raw(
        &self,
        uid: &str,
        body: Vec<u8>,
        content_type: &str,
        primary_key: Option<&str>,
    ) -> Result<Value> {
        let mut qs = String::new();
        if let Some(pk) = primary_key {
            qs = format!("?primaryKey={pk}");
        }
        let resp = self
            .http
            .put(self.url(&format!("/indexes/{uid}/documents{qs}")))
            .header(CONTENT_TYPE, content_type)
            .body(body)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn fetch_documents(&self, uid: &str, body: &Value) -> Result<Value> {
        self.post(&format!("/indexes/{uid}/documents/fetch"), body)
            .await
    }

    pub async fn delete_documents_by_filter(&self, uid: &str, filter: &str) -> Result<Value> {
        self.post(
            &format!("/indexes/{uid}/documents/delete"),
            &serde_json::json!({ "filter": filter }),
        )
        .await
    }

    pub async fn edit_documents(
        &self,
        uid: &str,
        function: &str,
        filter: Option<&str>,
    ) -> Result<Value> {
        let mut body = serde_json::json!({ "function": function });
        if let Some(f) = filter {
            body["filter"] = serde_json::json!(f);
        }
        self.post(&format!("/indexes/{uid}/documents/edit"), &body)
            .await
    }

    // ── Search ────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub async fn search(
        &self,
        uid: &str,
        query: &str,
        filter: Option<&str>,
        facets: Option<&[String]>,
        limit: Option<u64>,
        offset: Option<u64>,
        sort: Option<&[String]>,
        attributes_to_retrieve: Option<&[String]>,
        attributes_to_highlight: Option<&[String]>,
    ) -> Result<Value> {
        let mut body = serde_json::json!({ "q": query });
        if let Some(f) = filter {
            body["filter"] = serde_json::json!(f);
        }
        if let Some(f) = facets {
            body["facets"] = serde_json::json!(f);
        }
        if let Some(l) = limit {
            body["limit"] = serde_json::json!(l);
        }
        if let Some(o) = offset {
            body["offset"] = serde_json::json!(o);
        }
        if let Some(s) = sort {
            body["sort"] = serde_json::json!(s);
        }
        if let Some(a) = attributes_to_retrieve {
            body["attributesToRetrieve"] = serde_json::json!(a);
        }
        if let Some(a) = attributes_to_highlight {
            body["attributesToHighlight"] = serde_json::json!(a);
        }
        self.post(&format!("/indexes/{uid}/search"), &body).await
    }

    pub async fn multi_search(&self, queries: &Value) -> Result<Value> {
        self.post("/multi-search", &serde_json::json!({ "queries": queries }))
            .await
    }

    pub async fn facet_search(
        &self,
        uid: &str,
        facet_name: &str,
        facet_query: Option<&str>,
        filter: Option<&str>,
    ) -> Result<Value> {
        let mut body = serde_json::json!({ "facetName": facet_name });
        if let Some(q) = facet_query {
            body["facetQuery"] = serde_json::json!(q);
        }
        if let Some(f) = filter {
            body["filter"] = serde_json::json!(f);
        }
        self.post(&format!("/indexes/{uid}/facet-search"), &body)
            .await
    }

    // ── Settings ──────────────────────────────────────────────────

    pub async fn get_settings(&self, uid: &str) -> Result<Value> {
        self.get(&format!("/indexes/{uid}/settings")).await
    }

    pub async fn update_settings(&self, uid: &str, settings: &Value) -> Result<Value> {
        self.patch(&format!("/indexes/{uid}/settings"), settings)
            .await
    }

    pub async fn reset_settings(&self, uid: &str) -> Result<Value> {
        self.delete(&format!("/indexes/{uid}/settings")).await
    }

    // Settings sub-resources use PUT for most, PATCH for embedders/faceting/pagination/typo-tolerance
    const PATCH_SETTINGS: &'static [&'static str] =
        &["embedders", "faceting", "pagination", "typo-tolerance"];

    pub async fn get_setting(&self, uid: &str, sub: &str) -> Result<Value> {
        self.get(&format!("/indexes/{uid}/settings/{sub}")).await
    }

    pub async fn update_setting(&self, uid: &str, sub: &str, value: &Value) -> Result<Value> {
        let path = format!("/indexes/{uid}/settings/{sub}");
        if Self::PATCH_SETTINGS.contains(&sub) {
            self.patch(&path, value).await
        } else {
            self.put(&path, value).await
        }
    }

    pub async fn reset_setting(&self, uid: &str, sub: &str) -> Result<Value> {
        self.delete(&format!("/indexes/{uid}/settings/{sub}")).await
    }

    // ── Batches ───────────────────────────────────────────────────

    pub async fn list_batches(
        &self,
        limit: Option<u64>,
        from: Option<u64>,
        uids: Option<&str>,
        index_uids: Option<&str>,
        statuses: Option<&str>,
        types: Option<&str>,
    ) -> Result<Value> {
        let mut params = vec![];
        if let Some(l) = limit {
            params.push(format!("limit={l}"));
        }
        if let Some(f) = from {
            params.push(format!("from={f}"));
        }
        if let Some(u) = uids {
            params.push(format!("uids={u}"));
        }
        if let Some(i) = index_uids {
            params.push(format!("indexUids={i}"));
        }
        if let Some(s) = statuses {
            params.push(format!("statuses={s}"));
        }
        if let Some(t) = types {
            params.push(format!("types={t}"));
        }
        let qs = if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        };
        self.get(&format!("/batches{qs}")).await
    }

    pub async fn get_batch(&self, uid: u64) -> Result<Value> {
        self.get(&format!("/batches/{uid}")).await
    }

    // ── Logs ──────────────────────────────────────────────────────

    pub async fn update_log_stderr(&self, target: &str) -> Result<Value> {
        self.post("/logs/stderr", &serde_json::json!({ "target": target }))
            .await
    }

    pub async fn stream_logs(&self, target: &str, mode: Option<&str>) -> Result<Value> {
        let mut body = serde_json::json!({ "target": target });
        if let Some(m) = mode {
            body["mode"] = serde_json::json!(m);
        }
        self.post("/logs/stream", &body).await
    }

    pub async fn stop_log_stream(&self) -> Result<Value> {
        self.delete("/logs/stream").await
    }

    // ── Network ───────────────────────────────────────────────────

    pub async fn get_network(&self) -> Result<Value> {
        self.get("/network").await
    }

    pub async fn update_network(&self, body: &Value) -> Result<Value> {
        self.patch("/network", body).await
    }

    // ── Metrics ───────────────────────────────────────────────────

    pub async fn get_metrics_raw(&self) -> Result<String> {
        let resp = self.http.get(self.url("/metrics")).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("HTTP {}: {}", status, body);
        }
        Ok(body)
    }

    // ── Tasks ─────────────────────────────────────────────────────

    pub async fn list_tasks(&self, filters: &TaskFilters) -> Result<Value> {
        let qs = filters.to_query_string();
        self.get(&format!("/tasks{qs}")).await
    }

    pub async fn get_task(&self, task_id: u64) -> Result<Value> {
        self.get(&format!("/tasks/{task_id}")).await
    }

    pub async fn cancel_tasks(&self, filters: &TaskFilters) -> Result<Value> {
        let qs = filters.to_query_string();
        self.post(&format!("/tasks/cancel{qs}"), &serde_json::json!({}))
            .await
    }

    pub async fn delete_tasks(&self, filters: &TaskFilters) -> Result<Value> {
        let qs = filters.to_query_string();
        self.delete(&format!("/tasks{qs}")).await
    }

    pub async fn wait_for_task(&self, task_id: u64, timeout_ms: u64) -> Result<Value> {
        let start = std::time::Instant::now();
        loop {
            let task = self.get_task(task_id).await?;
            let status = task["status"].as_str().unwrap_or("unknown");
            match status {
                "succeeded" | "failed" | "canceled" => return Ok(task),
                _ => {
                    if start.elapsed().as_millis() as u64 > timeout_ms {
                        bail!("Task {} timed out after {}ms", task_id, timeout_ms);
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
            }
        }
    }

    // ── Keys ──────────────────────────────────────────────────────

    pub async fn list_keys(&self) -> Result<Value> {
        self.get("/keys").await
    }

    pub async fn get_key(&self, key: &str) -> Result<Value> {
        self.get(&format!("/keys/{key}")).await
    }

    pub async fn create_key(
        &self,
        description: Option<&str>,
        actions: &[String],
        indexes: &[String],
        expires_at: Option<&str>,
    ) -> Result<Value> {
        let mut body = serde_json::json!({
            "actions": actions,
            "indexes": indexes,
        });
        if let Some(d) = description {
            body["description"] = serde_json::json!(d);
        }
        if let Some(e) = expires_at {
            body["expiresAt"] = serde_json::json!(e);
        } else {
            body["expiresAt"] = serde_json::json!(null);
        }
        self.post("/keys", &body).await
    }

    pub async fn update_key(
        &self,
        key: &str,
        description: Option<&str>,
        name: Option<&str>,
    ) -> Result<Value> {
        let mut body = serde_json::json!({});
        if let Some(d) = description {
            body["description"] = serde_json::json!(d);
        }
        if let Some(n) = name {
            body["name"] = serde_json::json!(n);
        }
        self.patch(&format!("/keys/{key}"), &body).await
    }

    pub async fn delete_key(&self, key: &str) -> Result<Value> {
        self.delete(&format!("/keys/{key}")).await
    }

    // ── Dumps / Snapshots ─────────────────────────────────────────

    pub async fn create_dump(&self) -> Result<Value> {
        self.post("/dumps", &serde_json::json!({})).await
    }

    pub async fn create_snapshot(&self) -> Result<Value> {
        self.post("/snapshots", &serde_json::json!({})).await
    }

    // ── Export ─────────────────────────────────────────────────────

    pub async fn export(&self, body: &Value) -> Result<Value> {
        self.post("/export", body).await
    }

    // ── Experimental Features ─────────────────────────────────────

    pub async fn get_experimental_features(&self) -> Result<Value> {
        self.get("/experimental-features").await
    }

    pub async fn update_experimental_features(&self, features: &Value) -> Result<Value> {
        self.patch("/experimental-features", features).await
    }

    // ── Similar ───────────────────────────────────────────────────

    pub async fn similar(
        &self,
        uid: &str,
        id: &str,
        limit: Option<u64>,
        filter: Option<&str>,
    ) -> Result<Value> {
        let mut body = serde_json::json!({ "id": id });
        if let Some(l) = limit {
            body["limit"] = serde_json::json!(l);
        }
        if let Some(f) = filter {
            body["filter"] = serde_json::json!(f);
        }
        self.post(&format!("/indexes/{uid}/similar"), &body).await
    }

    // ── Chat Workspaces & Completions ────────────────────────────

    pub async fn list_chat_workspaces(&self) -> Result<Value> {
        self.get("/chats").await
    }

    pub async fn get_chat_settings(&self, workspace: &str) -> Result<Value> {
        self.get(&format!("/chats/{workspace}/settings")).await
    }

    pub async fn update_chat_settings(&self, workspace: &str, settings: &Value) -> Result<Value> {
        self.patch(&format!("/chats/{workspace}/settings"), settings)
            .await
    }

    pub async fn delete_chat_workspace(&self, workspace: &str) -> Result<Value> {
        self.delete(&format!("/chats/{workspace}")).await
    }

    pub async fn chat_completions(
        &self,
        workspace: &str,
        body: &Value,
    ) -> Result<reqwest::Response> {
        let resp = self
            .http
            .post(self.url(&format!("/chats/{workspace}/chat/completions")))
            .json(body)
            .send()
            .await?;
        Ok(resp)
    }

    // ── HTTP primitives ───────────────────────────────────────────

    async fn get(&self, path: &str) -> Result<Value> {
        let resp = self.http.get(self.url(path)).send().await?;
        Self::handle_response(resp).await
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Value> {
        let resp = self.http.post(self.url(path)).json(body).send().await?;
        Self::handle_response(resp).await
    }

    async fn put(&self, path: &str, body: &Value) -> Result<Value> {
        let resp = self.http.put(self.url(path)).json(body).send().await?;
        Self::handle_response(resp).await
    }

    async fn patch(&self, path: &str, body: &Value) -> Result<Value> {
        let resp = self.http.patch(self.url(path)).json(body).send().await?;
        Self::handle_response(resp).await
    }

    async fn delete(&self, path: &str) -> Result<Value> {
        let resp = self.http.delete(self.url(path)).send().await?;
        Self::handle_response(resp).await
    }

    async fn handle_response(resp: reqwest::Response) -> Result<Value> {
        let status = resp.status();
        let body = resp.text().await?;

        if body.is_empty() {
            if status.is_success() {
                return Ok(Value::Null);
            }
            bail!("HTTP {}: (empty response)", status);
        }

        let json: Value =
            serde_json::from_str(&body).with_context(|| format!("HTTP {}: {}", status, body))?;

        if !status.is_success() {
            let msg = json["message"].as_str().unwrap_or("Unknown API error");
            let code = json["code"].as_str().unwrap_or("unknown");
            bail!("API error ({}): {} [HTTP {}]", code, msg, status);
        }

        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_url_building() {
        let client = MeiliClient::new("http://localhost:7700", None).unwrap();
        assert_eq!(client.base_url, "http://localhost:7700");

        let client = MeiliClient::new("http://localhost:7700/", None).unwrap();
        assert_eq!(client.base_url, "http://localhost:7700");
    }

    #[test]
    fn test_client_with_api_key() {
        let client = MeiliClient::new("http://localhost:7700", Some("test_key")).unwrap();
        assert_eq!(client.base_url, "http://localhost:7700");
    }

    #[tokio::test]
    async fn test_health_against_running_instance() {
        let client = MeiliClient::new("http://localhost:7700", None).unwrap();
        let result = client.health().await;
        if let Ok(value) = result {
            assert_eq!(value["status"], "available");
        }
        // If server is not running, skip gracefully
    }
}
