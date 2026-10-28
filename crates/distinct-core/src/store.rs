//! Immutable store for records and evidence (contract section 3).
//!
//! The store manages:
//! - STATE.json: the sole authority pointer (atomically replaced)
//! - store/objects/<sha256>.json: immutable typed record revisions
//! - store/snapshots/<sha256>.json: immutable content snapshot manifests
//! - store/governance/<sha256>.json: immutable governance index
//! - evidence/objects/<sha256>: original or derivative bytes
//! - evidence/index/: immutable metadata records

use crate::hash::{self, HexHash};
use crate::record::RecordKind;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The STATE.json authority pointer (contract section 3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub schema_version: String,
    pub brand_id: String,
    pub content_snapshot_hash: Option<String>,
    pub governance_index_hash: Option<String>,
    pub generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub created_at: String,
}

/// A content snapshot manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: String,
    pub brand_id: String,
    pub records: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
}

/// A governance index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceIndex {
    pub schema_version: String,
    pub brand_id: String,
    pub records: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
}

/// Workspace paths for a brand.
#[derive(Debug, Clone)]
pub struct BrandWorkspace {
    pub root: PathBuf,
    pub brand_id: String,
}

impl BrandWorkspace {
    /// Create a new workspace reference.
    pub fn new(workspace_root: &Path, brand_id: &str) -> Self {
        Self {
            root: workspace_root.join(".distinct").join(brand_id),
            brand_id: brand_id.to_string(),
        }
    }

    pub fn state_path(&self) -> PathBuf {
        self.root.join("STATE.json")
    }

    pub fn objects_dir(&self) -> PathBuf {
        self.root.join("store/objects")
    }

    pub fn snapshots_dir(&self) -> PathBuf {
        self.root.join("store/snapshots")
    }

    pub fn governance_dir(&self) -> PathBuf {
        self.root.join("store/governance")
    }

    pub fn evidence_objects_dir(&self) -> PathBuf {
        self.root.join("evidence/objects")
    }

    pub fn evidence_index_dir(&self) -> PathBuf {
        self.root.join("evidence/index")
    }

    pub fn packets_dir(&self) -> PathBuf {
        self.root.join("packets")
    }

    /// Check if the brand is initialized.
    pub fn is_initialized(&self) -> bool {
        self.state_path().exists()
    }

    /// Read the current STATE.json.
    pub fn read_state(&self) -> Result<State, StoreError> {
        let contents = std::fs::read_to_string(self.state_path())
            .map_err(|e| StoreError::Io(e.to_string()))?;
        serde_json::from_str(&contents)
            .map_err(|e| StoreError::Parse(format!("corrupt STATE.json: {e}")))
    }

    /// Write STATE.json atomically with process-safe exclusive locking.
    pub fn write_state(&self, state: &State) -> Result<(), StoreError> {
        let json = serde_json::to_string_pretty(state)
            .map_err(|e| StoreError::Serialize(e.to_string()))?;

        // Acquire exclusive lock via .lock file
        let lock_path = self.root.join(".lock");
        let lock_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| StoreError::Io(format!("open lock file: {e}")))?;
        <std::fs::File as fs4::FileExt>::lock(&lock_file)
            .map_err(|e| StoreError::Io(format!("acquire lock: {e}")))?;

        // Write to temp, then atomic rename
        let tmp = self.state_path().with_extension("json.tmp");
        let write_result = std::fs::write(&tmp, &json);

        // Always release lock
        let _ = <std::fs::File as fs4::FileExt>::unlock(&lock_file);

        write_result.map_err(|e| StoreError::Io(format!("write temp STATE: {e}")))?;
        std::fs::rename(&tmp, self.state_path())
            .map_err(|e| StoreError::Io(format!("atomic rename STATE: {e}")))?;
        Ok(())
    }

    /// Store an immutable object. Returns its content hash.
    pub fn store_object(&self, record: &serde_json::Value) -> Result<HexHash, StoreError> {
        // Validate record id, kind, and privacy before storing.
        if let Some(id) = record["id"].as_str() {
            crate::id::validate_record_id(id)
                .map_err(|e| StoreError::InvalidRecord(format!("invalid id: {e}")))?;
        } else {
            return Err(StoreError::InvalidRecord("missing or non-string id".into()));
        }

        if let Some(kind_str) = record["kind"].as_str() {
            serde_json::from_str::<RecordKind>(&format!("\"{kind_str}\""))
                .map_err(|_| StoreError::InvalidRecord(format!("unknown kind: {kind_str}")))?;
        } else {
            return Err(StoreError::InvalidRecord(
                "missing or non-string kind".into(),
            ));
        }

        if let Some(privacy_str) = record["privacy"].as_str() {
            match privacy_str {
                "public" | "internal" | "confidential" | "restricted" => {}
                _ => {
                    return Err(StoreError::InvalidRecord(format!(
                        "invalid privacy: {privacy_str}"
                    )));
                }
            }
        } else {
            return Err(StoreError::InvalidRecord(
                "missing or non-string privacy".into(),
            ));
        }

        // Compute content hash (excluding derived fields)
        let mut hash_body = record.clone();
        if let Some(obj) = hash_body.as_object_mut() {
            obj.remove("content_hash");
            obj.remove("revision_id");
        }
        let content_hash =
            hash::content_hash(&hash_body).map_err(|e| StoreError::Hash(e.to_string()))?;

        // Add derived fields back
        let mut stored = record.clone();
        if let Some(obj) = stored.as_object_mut() {
            obj.insert(
                "content_hash".into(),
                serde_json::json!(content_hash.as_hex()),
            );
            obj.insert(
                "revision_id".into(),
                serde_json::json!(hash::revision_id(&content_hash)),
            );
        }

        let json = serde_json::to_string_pretty(&stored)
            .map_err(|e| StoreError::Serialize(e.to_string()))?;
        let path = self
            .objects_dir()
            .join(format!("{}.json", content_hash.as_hex()));

        // Immutable: if it already exists, that's fine (idempotent)
        if !path.exists() {
            std::fs::write(&path, &json)
                .map_err(|e| StoreError::Io(format!("write object: {e}")))?;
        }

        Ok(content_hash)
    }

    /// Store evidence bytes. Returns the raw byte hash.
    pub fn store_evidence(&self, bytes: &[u8], source_id: &str) -> Result<HexHash, StoreError> {
        // source_id becomes a file name below; reject anything that is not a plain record id.
        crate::id::validate_record_id(source_id)
            .map_err(|e| StoreError::InvalidRecord(format!("invalid source_id: {e}")))?;
        let byte_hash = hash::hash_bytes(bytes);
        let path = self.evidence_objects_dir().join(byte_hash.as_hex());

        if !path.exists() {
            std::fs::write(&path, bytes)
                .map_err(|e| StoreError::Io(format!("write evidence: {e}")))?;
        }

        // Write source metadata
        let meta = serde_json::json!({
            "source_id": source_id,
            "byte_hash": byte_hash.as_hex(),
            "byte_count": bytes.len(),
            "stored_at": chrono_now(),
        });
        let meta_path = self
            .evidence_index_dir()
            .join(format!("{}.json", source_id));
        std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap())
            .map_err(|e| StoreError::Io(format!("write evidence index: {e}")))?;

        Ok(byte_hash)
    }

    /// List all stored objects, optionally filtered by kind.
    pub fn list_objects(
        &self,
        kind_filter: Option<RecordKind>,
    ) -> Result<Vec<serde_json::Value>, StoreError> {
        let dir = self.objects_dir();
        let mut results = Vec::new();

        if !dir.exists() {
            return Ok(results);
        }

        for entry in std::fs::read_dir(&dir).map_err(|e| StoreError::Io(e.to_string()))? {
            let entry = entry.map_err(|e| StoreError::Io(e.to_string()))?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    if let Ok(record) = serde_json::from_str::<serde_json::Value>(&contents) {
                        let matches =
                            kind_filter.is_none_or(|k| record["kind"].as_str() == Some(k.as_str()));
                        if matches {
                            results.push(record);
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Update the content snapshot after record changes.
    pub fn update_snapshot(&self, state: &mut State) -> Result<(), StoreError> {
        let objects = self.list_objects(None)?;
        let mut records: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, String>,
        > = std::collections::BTreeMap::new();

        for obj in &objects {
            let kind = obj["kind"].as_str().unwrap_or("unknown").to_string();
            let id = obj["id"].as_str().unwrap_or("unknown").to_string();
            let hash = obj["content_hash"].as_str().unwrap_or("").to_string();

            // Only content kinds go in the content snapshot
            if let Ok(k) = serde_json::from_value::<RecordKind>(obj["kind"].clone()) {
                if k.is_content() {
                    records.entry(kind).or_default().insert(id, hash);
                }
            }
        }

        let snapshot = Snapshot {
            schema_version: crate::record::SCHEMA_VERSION.to_string(),
            brand_id: self.brand_id.clone(),
            records,
        };

        let json = serde_json::to_string_pretty(&snapshot)
            .map_err(|e| StoreError::Serialize(e.to_string()))?;
        let snapshot_hash = hash::hash_raw(crate::hash::DOMAIN_SNAPSHOT, json.as_bytes());

        let path = self
            .snapshots_dir()
            .join(format!("{}.json", snapshot_hash.as_hex()));
        if !path.exists() {
            std::fs::write(&path, &json)
                .map_err(|e| StoreError::Io(format!("write snapshot: {e}")))?;
        }

        state.content_snapshot_hash = Some(snapshot_hash.as_hex().to_string());
        state.generation += 1;

        Ok(())
    }

    /// Update the governance index after governance record changes.
    pub fn update_governance_index(&self, state: &mut State) -> Result<(), StoreError> {
        let objects = self.list_objects(None)?;
        let mut records: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, String>,
        > = std::collections::BTreeMap::new();

        for obj in &objects {
            let kind = obj["kind"].as_str().unwrap_or("unknown").to_string();
            let id = obj["id"].as_str().unwrap_or("unknown").to_string();
            let hash_val = obj["content_hash"].as_str().unwrap_or("").to_string();

            if let Ok(k) = serde_json::from_value::<RecordKind>(obj["kind"].clone()) {
                if k.is_governance() {
                    records.entry(kind).or_default().insert(id, hash_val);
                }
            }
        }

        let index = GovernanceIndex {
            schema_version: crate::record::SCHEMA_VERSION.to_string(),
            brand_id: self.brand_id.clone(),
            records,
        };

        let json = serde_json::to_string_pretty(&index)
            .map_err(|e| StoreError::Serialize(e.to_string()))?;
        let gov_hash = hash::hash_raw(crate::hash::DOMAIN_GOVERNANCE, json.as_bytes());

        let path = self
            .governance_dir()
            .join(format!("{}.json", gov_hash.as_hex()));
        if !path.exists() {
            std::fs::write(&path, &json)
                .map_err(|e| StoreError::Io(format!("write governance index: {e}")))?;
        }

        state.governance_index_hash = Some(gov_hash.as_hex().to_string());

        Ok(())
    }
}

/// Store errors.
#[derive(Debug, Clone)]
pub enum StoreError {
    Io(String),
    Parse(String),
    Serialize(String),
    Hash(String),
    Conflict(String),
    InvalidRecord(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Parse(e) => write!(f, "parse error: {e}"),
            Self::Serialize(e) => write!(f, "serialization error: {e}"),
            Self::Hash(e) => write!(f, "hash error: {e}"),
            Self::Conflict(e) => write!(f, "conflict: {e}"),
            Self::InvalidRecord(e) => write!(f, "invalid record: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Current UTC timestamp in RFC 3339 format.
pub fn chrono_now() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_rfc3339(secs)
}

/// Format Unix seconds as RFC 3339 UTC (`YYYY-MM-DDTHH:MM:SSZ`).
///
/// Uses the Howard Hinnant days-to-civil algorithm for the date portion.
fn format_rfc3339(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

/// Convert days since Unix epoch (1970-01-01) to (year, month, day).
fn days_to_ymd(mut days: i64) -> (i64, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    days += 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u32; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month index [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_workspace() -> (PathBuf, BrandWorkspace) {
        let dir = std::env::temp_dir().join(format!("distinct-store-test-{}", rand_suffix()));
        fs::create_dir_all(&dir).unwrap();
        let ws = BrandWorkspace::new(&dir, "testbrand");
        // Create directory structure
        fs::create_dir_all(ws.objects_dir()).unwrap();
        fs::create_dir_all(ws.snapshots_dir()).unwrap();
        fs::create_dir_all(ws.governance_dir()).unwrap();
        fs::create_dir_all(ws.evidence_objects_dir()).unwrap();
        fs::create_dir_all(ws.evidence_index_dir()).unwrap();
        (dir, ws)
    }

    fn rand_suffix() -> String {
        use std::time::SystemTime;
        let d = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        format!("{}", d.as_nanos())
    }

    #[test]
    fn workspace_paths() {
        let ws = BrandWorkspace::new(Path::new("/tmp/test"), "acme");
        assert!(ws.state_path().ends_with(".distinct/acme/STATE.json"));
        assert!(ws.objects_dir().ends_with(".distinct/acme/store/objects"));
    }

    #[test]
    fn store_and_read_state() {
        let (dir, ws) = temp_workspace();
        let state = State {
            schema_version: "1.0.0".into(),
            brand_id: "testbrand".into(),
            content_snapshot_hash: None,
            governance_index_hash: None,
            generation: 0,
            name: Some("Test".into()),
            created_at: "2026-10-26T00:00:00Z".into(),
        };
        ws.write_state(&state).unwrap();
        let read = ws.read_state().unwrap();
        assert_eq!(read.brand_id, "testbrand");
        assert_eq!(read.generation, 0);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_object_idempotent() {
        let (dir, ws) = temp_workspace();
        let record = serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "project",
            "id": "main",
            "brand_id": "testbrand",
            "privacy": "public",
            "data": {"name": "test"}
        });
        let h1 = ws.store_object(&record).unwrap();
        let h2 = ws.store_object(&record).unwrap();
        assert_eq!(h1, h2, "storing same object twice must be idempotent");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_evidence_bytes() {
        let (dir, ws) = temp_workspace();
        let bytes = b"hello evidence";
        let h = ws.store_evidence(bytes, "src-001").unwrap();
        assert_eq!(h.as_hex().len(), 64);
        // Verify file exists
        assert!(ws.evidence_objects_dir().join(h.as_hex()).exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_objects_by_kind() {
        let (dir, ws) = temp_workspace();
        let proj = serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "project",
            "id": "main",
            "brand_id": "testbrand",
            "privacy": "public",
            "data": {}
        });
        let plat = serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "platform",
            "id": "main",
            "brand_id": "testbrand",
            "privacy": "public",
            "data": {}
        });
        ws.store_object(&proj).unwrap();
        ws.store_object(&plat).unwrap();

        let all = ws.list_objects(None).unwrap();
        assert_eq!(all.len(), 2);

        let projects = ws.list_objects(Some(RecordKind::Project)).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0]["kind"], "project");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn format_rfc3339_epoch_zero() {
        assert_eq!(format_rfc3339(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn format_rfc3339_leap_y2k() {
        // 951782400 seconds = 2000-02-29T00:00:00Z (leap day)
        assert_eq!(format_rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn update_snapshot_advances_generation() {
        let (dir, ws) = temp_workspace();
        let record = serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "project",
            "id": "main",
            "brand_id": "testbrand",
            "privacy": "public",
            "data": {"name": "test"}
        });
        ws.store_object(&record).unwrap();

        let mut state = State {
            schema_version: "1.0.0".into(),
            brand_id: "testbrand".into(),
            content_snapshot_hash: None,
            governance_index_hash: None,
            generation: 0,
            name: None,
            created_at: "2026-10-26T00:00:00Z".into(),
        };
        ws.update_snapshot(&mut state).unwrap();
        assert!(state.content_snapshot_hash.is_some());
        assert_eq!(state.generation, 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_object_rejects_missing_privacy() {
        let (dir, ws) = temp_workspace();
        let record = serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "project",
            "id": "main",
            "brand_id": "testbrand",
            "data": {}
        });
        let err = ws.store_object(&record).unwrap_err();
        assert!(
            matches!(err, StoreError::InvalidRecord(_)),
            "expected InvalidRecord, got {err}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_object_rejects_invalid_id() {
        let (dir, ws) = temp_workspace();
        let record = serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "project",
            "id": "../../evil",
            "brand_id": "testbrand",
            "privacy": "public",
            "data": {}
        });
        let err = ws.store_object(&record).unwrap_err();
        assert!(
            matches!(err, StoreError::InvalidRecord(_)),
            "expected InvalidRecord for traversal id, got {err}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_object_rejects_unknown_kind() {
        let (dir, ws) = temp_workspace();
        let record = serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "not_a_real_kind",
            "id": "main",
            "brand_id": "testbrand",
            "privacy": "public",
            "data": {}
        });
        let err = ws.store_object(&record).unwrap_err();
        assert!(
            matches!(err, StoreError::InvalidRecord(_)),
            "expected InvalidRecord for unknown kind, got {err}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_object_rejects_invalid_privacy() {
        let (dir, ws) = temp_workspace();
        let record = serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "project",
            "id": "main",
            "brand_id": "testbrand",
            "privacy": "top_secret",
            "data": {}
        });
        let err = ws.store_object(&record).unwrap_err();
        assert!(
            matches!(err, StoreError::InvalidRecord(_)),
            "expected InvalidRecord for invalid privacy, got {err}"
        );
        fs::remove_dir_all(&dir).ok();
    }
}
