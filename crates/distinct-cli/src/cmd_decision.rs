//! `decision record` / `decision confirm` (D-M owner confirmation).
//!
//! `record` validates a decision and stores it as *pending*; a one-time code
//! is written only to the controlling terminal. `confirm` checks the code and
//! only then stores the real decision with a `terminal_code` attestation.
//!
//! The code is 50 bits; the stored verifier is salted and iterated (200k
//! sha256 rounds), so offline guessing within the 15-minute expiry is infeasible.
//! Limit: this stops an agent from fabricating approvals by following
//! instructions. A host that allocates its own pty can read the code, so this
//! is a safeguard, not cryptographic proof of who the owner is.

use crate::dispatch::{self, write_envelope, write_error, CliArgs};
use distinct_core::store::{chrono_now, BrandWorkspace};
use distinct_core::Exit;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;

const CODE_TTL_SECS: u64 = 900;
const MAX_ATTEMPTS: u64 = 5;

#[cfg(windows)]
const TERMINAL_DEVICE: &str = "CONOUT$";
#[cfg(not(windows))]
const TERMINAL_DEVICE: &str = "/dev/tty";

pub(crate) fn cmd_decision<O: Write>(args: &CliArgs, out: &mut O) -> Exit {
    match args.rest.first().map(String::as_str) {
        Some("record") => record(args, out),
        Some("confirm") => confirm(args, out),
        _ => write_error(
            out,
            "decision",
            "usage: decision record --file <f> --owner <id> | decision confirm --id <pending_id> --code <code>",
            Exit::Invalid,
        ),
    }
}

fn flag<'a>(args: &'a CliArgs, name: &str) -> Option<&'a str> {
    args.rest
        .iter()
        .position(|a| a == name)
        .and_then(|i| args.rest.get(i + 1))
        .map(String::as_str)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── randomness, hashing, terminal ────────────────────────────────────

/// 32 bytes mixed from OS-seeded `RandomState`, pid, time and `tag`, via sha256.
fn random_bytes(tag: &str) -> [u8; 32] {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut sha = Sha256::new();
    for _ in 0..4 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u32(std::process::id());
        h.write(tag.as_bytes());
        h.write_u128(nanos);
        sha.update(h.finish().to_le_bytes());
    }
    sha.update(tag.as_bytes());
    sha.update(std::process::id().to_le_bytes());
    sha.update(nanos.to_le_bytes());
    sha.finalize().into()
}

/// Crockford base32 (no I, L, O, U).
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CODE_LEN: usize = 10;
/// Release iteration count for the stored verifier.
const KDF_ITERATIONS: u32 = 200_000;

/// 10 Crockford base32 characters (50 bits), normalized (no hyphen).
fn generate_code(pending_id: &str) -> String {
    let b = random_bytes(pending_id);
    let mut n = [0u8; 8];
    n.copy_from_slice(&b[..8]);
    let v = u64::from_le_bytes(n);
    (0..CODE_LEN)
        .map(|i| ALPHABET[((v >> (5 * i)) & 31) as usize] as char)
        .collect()
}

/// Display form `XXXXX-XXXXX`.
fn display_code(code: &str) -> String {
    format!("{}-{}", &code[..5], &code[5..])
}

/// Case-insensitive, hyphen-optional; I/L -> 1, O -> 0. `None` if malformed.
fn normalize_code(input: &str) -> Option<String> {
    let out: String = input
        .chars()
        .filter(|c| *c != '-')
        .map(|c| match c.to_ascii_uppercase() {
            'I' | 'L' => '1',
            'O' => '0',
            u => u,
        })
        .collect();
    (out.len() == CODE_LEN && out.bytes().all(|b| ALPHABET.contains(&b))).then_some(out)
}

fn new_pending_id() -> String {
    let b = random_bytes("pending-id");
    format!("pd-{}", to_hex(&b[..6]))
}

fn to_hex(b: &[u8]) -> String {
    use std::fmt::Write as _;
    b.iter().fold(String::new(), |mut acc, x| {
        let _ = write!(acc, "{x:02x}");
        acc
    })
}

/// Iteration count: fixed in release builds; debug builds may lower it via
/// `DISTINCT_TEST_KDF_ITERATIONS` so tests stay fast.
#[cfg(debug_assertions)]
fn kdf_iterations() -> u32 {
    std::env::var("DISTINCT_TEST_KDF_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(KDF_ITERATIONS)
}

#[cfg(not(debug_assertions))]
fn kdf_iterations() -> u32 {
    KDF_ITERATIONS
}

/// h = sha256(salt || code || id); then `iterations` times h = sha256(h || salt).
fn verifier(salt: &[u8], iterations: u32, code: &str, pending_id: &str) -> String {
    let mut sha = Sha256::new();
    sha.update(salt);
    sha.update(code.as_bytes());
    sha.update(pending_id.as_bytes());
    let mut h: [u8; 32] = sha.finalize().into();
    for _ in 0..iterations {
        let mut sha = Sha256::new();
        sha.update(h);
        sha.update(salt);
        h = sha.finalize().into();
    }
    to_hex(&h)
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Constant-time equality for equal-length byte slices (false on length mismatch).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Debug builds only: `DISTINCT_TEST_CONFIRM_CODE` replaces the random code
/// and skips the terminal write.
#[cfg(debug_assertions)]
fn test_code_override() -> Option<String> {
    std::env::var("DISTINCT_TEST_CONFIRM_CODE")
        .ok()
        .and_then(|c| normalize_code(&c))
}

#[cfg(not(debug_assertions))]
fn test_code_override() -> Option<String> {
    None
}

/// Debug builds only: `DISTINCT_TEST_TTY_PATH` replaces the terminal device.
#[cfg(debug_assertions)]
fn terminal_path() -> PathBuf {
    std::env::var_os("DISTINCT_TEST_TTY_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TERMINAL_DEVICE))
}

#[cfg(not(debug_assertions))]
fn terminal_path() -> PathBuf {
    PathBuf::from(TERMINAL_DEVICE)
}

fn write_to_terminal(msg: &str) -> std::io::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .open(terminal_path())?;
    f.write_all(msg.as_bytes())?;
    f.flush()
}

fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

// ── pending store ────────────────────────────────────────────────────

fn pending_path(ws: &BrandWorkspace, id: &str) -> PathBuf {
    ws.root.join("pending").join(format!("{id}.json"))
}

fn write_pending(path: &std::path::Path, v: &Value) -> Result<(), String> {
    let dir = path.parent().ok_or("bad pending path")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("create pending dir: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| format!("write pending: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename pending: {e}"))
}

// ── validation shared by record and confirm ──────────────────────────

fn validate_target(ws: &BrandWorkspace, record: &Value) -> Result<(), String> {
    let target = &record["data"]["target"];
    let (Some(k), Some(id), Some(h)) = (
        target["kind"].as_str(),
        target["id"].as_str(),
        target["content_hash"].as_str(),
    ) else {
        return Err("data.target must include {kind, id, content_hash}".into());
    };
    let objects = ws.list_objects(None).map_err(|e| e.to_string())?;
    let current = objects
        .iter()
        .find(|o| o["kind"].as_str() == Some(k) && o["id"].as_str() == Some(id));
    match current {
        Some(rec) if rec["content_hash"].as_str() == Some(h) => Ok(()),
        Some(rec) => Err(format!(
            "conflict: target {k}/{id} content_hash {h} does not match current {}",
            rec["content_hash"].as_str().unwrap_or("?")
        )),
        None => Err(format!("target {k}/{id} not found")),
    }
}

fn load_and_validate(args: &CliArgs, brand: &str, ws: &BrandWorkspace) -> Result<Value, String> {
    let file_path = flag(args, "--file").ok_or("--file <path> required")?;
    let json_text =
        std::fs::read_to_string(file_path).map_err(|e| format!("cannot read {file_path}: {e}"))?;
    distinct_core::dup_keys::reject_duplicate_keys(&json_text).map_err(|e| e.to_string())?;
    let record: Value =
        serde_json::from_str(&json_text).map_err(|e| format!("invalid JSON: {e}"))?;

    if record["kind"].as_str() != Some("decision") {
        return Err(format!(
            "expected kind 'decision', got '{}'",
            record["kind"].as_str().unwrap_or("?")
        ));
    }
    if record["created_by"]["kind"].as_str() == Some("agent") {
        return Err("agent actors are not permitted for decision records".into());
    }
    if record.get("attestation").is_some() {
        return Err("attestation is set only by decision confirm".into());
    }
    if record["brand_id"].as_str() != Some(brand) {
        return Err(format!(
            "record brand_id '{}' does not match --brand '{brand}'",
            record["brand_id"].as_str().unwrap_or("?")
        ));
    }
    if !ws.is_initialized() {
        return Err("brand not initialized".into());
    }
    validate_target(ws, &record)?;
    Ok(record)
}

// ── decision record ──────────────────────────────────────────────────

fn record<O: Write>(args: &CliArgs, out: &mut O) -> Exit {
    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "decision", &msg, Exit::Invalid),
    };
    let Some(owner) = flag(args, "--owner") else {
        return write_error(
            out,
            "decision",
            "--owner <id> is required for decision records",
            Exit::Invalid,
        );
    };
    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    let record = match load_and_validate(args, brand, &ws) {
        Ok(r) => r,
        Err(e) => return write_error(out, "decision", &e, Exit::Invalid),
    };

    let pending_id = new_pending_id();
    let (code, hooked) = match test_code_override() {
        Some(c) => (c, true),
        None => (generate_code(&pending_id), false),
    };
    if !hooked {
        let msg = format!(
            "\nDistinct owner confirmation for pending decision {pending_id} \
             (brand {brand}, owner {}, record {}). One-time code: {}\n\
             Give this code to the agent only if you approve.\n",
            clean(owner),
            clean(record["id"].as_str().unwrap_or("?")),
            display_code(&code),
        );
        if write_to_terminal(&msg).is_err() {
            return write_error(
                out,
                "decision",
                "owner_terminal_unavailable: cannot open the controlling terminal to show the owner code; nothing was stored",
                Exit::Unavailable,
            );
        }
    }

    let salt = &random_bytes(&format!("salt-{pending_id}"))[..16];
    let iterations = kdf_iterations();
    let created = now_secs();
    let expires_at = created + CODE_TTL_SECS;
    let pending = json!({
        "pending_id": pending_id,
        "brand_id": brand,
        "owner": owner,
        "created_at": chrono_now(),
        "expires_at": expires_at,
        "attempts": 0,
        "salt": to_hex(salt),
        "iterations": iterations,
        "verifier": verifier(salt, iterations, &code, &pending_id),
        "record": record,
    });
    if let Err(e) = write_pending(&pending_path(&ws, &pending_id), &pending) {
        return write_error(out, "decision", &e, Exit::Invalid);
    }
    let result = json!({
        "brand_id": brand,
        "status": "pending_owner_confirmation",
        "pending_id": pending_id,
        "owner": owner,
        "expires_at": expires_at,
        "max_attempts": MAX_ATTEMPTS,
        "message": "A one-time owner code is on the owner's terminal. Ask the owner for it, then run: decision confirm --id <pending_id> --code <code>. Do not guess.",
    });
    write_envelope(out, "decision", result, vec![], vec![])
}

// ── decision confirm ─────────────────────────────────────────────────

fn confirm<O: Write>(args: &CliArgs, out: &mut O) -> Exit {
    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "decision", &msg, Exit::Invalid),
    };
    let (Some(id), Some(code)) = (flag(args, "--id"), flag(args, "--code")) else {
        return write_error(
            out,
            "decision",
            "usage: decision confirm --id <pending_id> --code <code>",
            Exit::Invalid,
        );
    };
    if let Err(e) = distinct_core::id::validate_record_id(id) {
        return write_error(
            out,
            "decision",
            &format!("invalid pending id: {e}"),
            Exit::Invalid,
        );
    }
    let Some(code) = normalize_code(code) else {
        return write_error(
            out,
            "decision",
            "code must be 10 characters (XXXXX-XXXXX)",
            Exit::Invalid,
        );
    };
    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "decision", "brand not initialized", Exit::Invalid);
    }
    let path = pending_path(&ws, id);
    let mut pending: Value = match std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
    {
        Some(v) => v,
        None => return write_error(out, "decision", "pending decision not found", Exit::Invalid),
    };
    if pending["brand_id"].as_str() != Some(brand) || pending["pending_id"].as_str() != Some(id) {
        return write_error(out, "decision", "pending decision not found", Exit::Invalid);
    }

    let attempts = pending["attempts"].as_u64().unwrap_or(MAX_ATTEMPTS);
    if now_secs() >= pending["expires_at"].as_u64().unwrap_or(0) {
        let _ = std::fs::remove_file(&path);
        return write_error(
            out,
            "decision",
            "pending decision expired; record it again",
            Exit::Unavailable,
        );
    }
    if attempts >= MAX_ATTEMPTS {
        let _ = std::fs::remove_file(&path);
        return write_error(
            out,
            "decision",
            "too many attempts; record the decision again",
            Exit::Unavailable,
        );
    }

    // Count the attempt before comparing so a crash cannot grant a free guess.
    pending["attempts"] = json!(attempts + 1);
    if let Err(e) = write_pending(&path, &pending) {
        return write_error(out, "decision", &e, Exit::Invalid);
    }
    let salt = pending["salt"].as_str().and_then(from_hex);
    let iterations = pending["iterations"].as_u64().unwrap_or(0);
    let expected = pending["verifier"].as_str().unwrap_or("");
    let ok = match (salt, u32::try_from(iterations)) {
        (Some(salt), Ok(it)) if it > 0 => ct_eq(
            verifier(&salt, it, &code, id).as_bytes(),
            expected.as_bytes(),
        ),
        _ => false,
    };
    if !ok {
        std::thread::sleep(std::time::Duration::from_secs(1));
        return write_error(
            out,
            "decision",
            &format!(
                "incorrect code; {} attempt(s) remaining",
                MAX_ATTEMPTS - attempts - 1
            ),
            Exit::Invalid,
        );
    }

    let mut record = pending["record"].clone();
    if let Err(e) = validate_target(&ws, &record) {
        return write_error(out, "decision", &e, Exit::Invalid);
    }
    record["attestation"] = json!({"method": "terminal_code", "confirmed_at": chrono_now()});

    let hash = match ws.store_object(&record) {
        Ok(h) => h,
        Err(e) => return write_error(out, "decision", &e.to_string(), Exit::Invalid),
    };
    let sync = (|| -> Result<(), String> {
        let mut state = ws.read_state().map_err(|e| e.to_string())?;
        ws.update_snapshot(&mut state)
            .map_err(|e| format!("update snapshot: {e}"))?;
        ws.update_governance_index(&mut state)
            .map_err(|e| format!("update governance index: {e}"))?;
        ws.write_state(&state)
            .map_err(|e| format!("write state: {e}"))
    })();
    if let Err(e) = sync {
        return write_error(out, "decision", &e, Exit::Invalid);
    }
    let _ = std::fs::remove_file(&path);

    let result = json!({
        "brand_id": brand,
        "content_hash": hash.as_hex(),
        "revision_id": distinct_core::hash::revision_id(&hash),
        "kind": "decision",
        "id": record["id"],
        "owner": pending["owner"],
        "attestation": record["attestation"],
    });
    write_envelope(out, "decision", result, vec![], vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_eq_basics() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"ab"));
    }

    #[test]
    fn codes_are_ten_crockford_chars() {
        let c = generate_code("pd-x");
        assert_eq!(c.len(), 10);
        assert_eq!(normalize_code(&c).as_deref(), Some(c.as_str()));
        assert_eq!(display_code(&c).len(), 11);
    }

    #[test]
    fn normalize_accepts_case_hyphen_and_confusables() {
        assert_eq!(normalize_code("abcde-fghjk").as_deref(), Some("ABCDEFGHJK"));
        assert_eq!(normalize_code("O1I1L-0O0o0").as_deref(), Some("0111100000"));
        assert!(normalize_code("12345").is_none());
        assert!(normalize_code("UUUUUUUUUU").is_none());
        assert!(normalize_code("1234-5678-9A").is_some());
    }

    #[test]
    fn verifier_binds_salt_code_and_id() {
        let v = verifier(b"salt1", 3, "ABCDEFGHJK", "pd-a");
        assert_ne!(v, verifier(b"salt2", 3, "ABCDEFGHJK", "pd-a"));
        assert_ne!(v, verifier(b"salt1", 3, "ABCDEFGHJM", "pd-a"));
        assert_ne!(v, verifier(b"salt1", 3, "ABCDEFGHJK", "pd-b"));
        assert_ne!(v, verifier(b"salt1", 4, "ABCDEFGHJK", "pd-a"));
    }

    #[test]
    fn release_default_is_200k() {
        assert_eq!(KDF_ITERATIONS, 200_000);
        #[cfg(not(debug_assertions))]
        assert_eq!(kdf_iterations(), 200_000);
    }

    #[test]
    fn pending_ids_are_valid_record_ids() {
        let id = new_pending_id();
        assert!(distinct_core::id::validate_record_id(&id).is_ok());
    }

    /// The env hook must exist only behind `cfg(debug_assertions)`.
    #[test]
    fn hook_is_cfg_gated_in_source() {
        let src = include_str!("cmd_decision.rs");
        let gate = ["#[cfg(not(debug", "_assertions))]\nfn test_code_override"].concat();
        assert!(src.contains(&gate));
        let dbg = ["#[cfg(debug", "_assertions)]\nfn test_code_override"].concat();
        assert!(src.contains(&dbg));
        let kdf = ["#[cfg(not(debug", "_assertions))]\nfn kdf_iterations"].concat();
        assert!(src.contains(&kdf));
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn release_ignores_hook() {
        std::env::set_var("DISTINCT_TEST_CONFIRM_CODE", "ABCDE-FGHJK");
        assert!(test_code_override().is_none());
    }
}
