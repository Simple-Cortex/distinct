//! CLI command dispatch for `distinct-rs` (contract section 5).
//!
//! WO-02 adds the full command matrix: init, status, capabilities,
//! record import, evidence import, query, packet, gate, verify, export.
//! Each command validates input, operates on the workspace, and returns
//! the standard JSON envelope.

use distinct_core::{Exit, ENGINE_VERSION, PROTOCOL_VERSION};
use std::io::Write;

pub const HELP: &str = "\
distinct-rs: native companion for the Distinct brand skill

Usage: distinct-rs <command> [options]

Commands:
  --version, --help       Print version or help
  protocol                Probe protocol (hidden)
  init                    Initialize a brand project
  status                  Read STATE.json (name, generation, snapshot/governance hashes)
  capabilities            List native capabilities
  record import           Import a content record (governance kinds rejected)
  evidence import         Import evidence payloads
  query                   List stored records by kind (metadata only, all revisions)
  packet                  Create a bounded task packet
  gate                    Check readiness for a scope
  verify                  Rehash and verify state (exit 1 when state_valid is false)
  export                  Export a draft kit

Global options:
  --workspace <path>      Brand workspace (default: cwd)
  --brand <id>            Brand ID (required for brand-scoped ops)
  --format <fmt>          Output format: compact|json|human (default: compact)
  --max-bytes <n>         Response size bound (default: 16384)

Exit codes: 0=ok, 1=check-failed, 2=unavailable, 3=invalid-input
";

/// Parsed CLI arguments.
pub struct CliArgs {
    pub command: String,
    pub workspace: String,
    pub brand: Option<String>,
    pub format: OutputFormat,
    pub max_bytes: usize,
    pub rest: Vec<String>,
}

/// Output format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum OutputFormat {
    Compact,
    Json,
    Human,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Compact
    }
}

/// Parse CLI arguments from raw argv (without program name).
pub fn parse_args(args: &[String]) -> Result<CliArgs, String> {
    let mut command = String::new();
    let mut workspace = String::from(".");
    let mut brand: Option<String> = None;
    let mut format = OutputFormat::Compact;
    let mut max_bytes: usize = 16384;
    let mut rest = Vec::new();
    let mut i = 0;

    // First non-flag arg is the command
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "-h" | "--help" => {
                return Ok(CliArgs {
                    command: "help".into(),
                    workspace,
                    brand,
                    format,
                    max_bytes,
                    rest,
                })
            }
            "-V" | "--version" => {
                return Ok(CliArgs {
                    command: "version".into(),
                    workspace,
                    brand,
                    format,
                    max_bytes,
                    rest,
                })
            }
            "--workspace" => {
                i += 1;
                workspace = args.get(i).ok_or("--workspace requires a value")?.clone();
            }
            "--brand" => {
                i += 1;
                let b = args.get(i).ok_or("--brand requires a value")?.clone();
                distinct_core::id::validate_brand_id(&b)
                    .map_err(|e| format!("invalid brand ID: {e}"))?;
                brand = Some(b);
            }
            "--format" => {
                i += 1;
                format = match args.get(i).map(|s| s.as_str()) {
                    Some("compact") => OutputFormat::Compact,
                    Some("json") => OutputFormat::Json,
                    Some("human") => OutputFormat::Human,
                    Some(other) => return Err(format!("unknown format: {other}")),
                    None => return Err("--format requires a value".into()),
                };
            }
            "--max-bytes" => {
                i += 1;
                let v = args.get(i).ok_or("--max-bytes requires a value")?;
                max_bytes = v.parse().map_err(|_| "invalid --max-bytes value")?;
                if max_bytes < 1024 {
                    return Err("--max-bytes minimum is 1024".into());
                }
            }
            _ if command.is_empty() => command = a.clone(),
            _ => rest.push(a.clone()),
        }
        i += 1;
    }

    if command.is_empty() {
        command = "help".into();
    }

    Ok(CliArgs {
        command,
        workspace,
        brand,
        format,
        max_bytes,
        rest,
    })
}

/// The standard JSON envelope for machine output (contract section 5).
#[derive(serde::Serialize)]
pub struct Envelope<R: serde::Serialize> {
    pub protocol_version: &'static str,
    pub engine_version: &'static str,
    pub execution: Execution,
    pub result: R,
    pub limitations: Vec<String>,
    pub changed: Vec<String>,
    pub error: Option<String>,
}

/// What was executed.
#[derive(serde::Serialize)]
pub struct Execution {
    pub engine: &'static str,
    pub command: String,
}

/// Write the standard envelope to stdout.
pub fn write_envelope<R: serde::Serialize, O: Write>(
    out: &mut O,
    command: &str,
    result: R,
    limitations: Vec<String>,
    changed: Vec<String>,
) -> Exit {
    let env = Envelope {
        protocol_version: PROTOCOL_VERSION,
        engine_version: ENGINE_VERSION,
        execution: Execution {
            engine: "native-rust",
            command: command.into(),
        },
        result,
        limitations,
        changed,
        error: None,
    };
    match serde_json::to_string(&env) {
        Ok(json) => {
            let _ = writeln!(out, "{json}");
            Exit::Ok
        }
        Err(e) => {
            let _ = writeln!(out, "{{\"error\":\"serialization failed: {e}\"}}");
            Exit::Invalid
        }
    }
}

/// Write an error envelope.
pub fn write_error<O: Write>(out: &mut O, command: &str, msg: &str, exit: Exit) -> Exit {
    let env = Envelope {
        protocol_version: PROTOCOL_VERSION,
        engine_version: ENGINE_VERSION,
        execution: Execution {
            engine: "native-rust",
            command: command.into(),
        },
        result: serde_json::Value::Null,
        limitations: vec![],
        changed: vec![],
        error: Some(msg.into()),
    };
    let _ = writeln!(out, "{}", serde_json::to_string(&env).unwrap_or_default());
    exit
}

/// Known commands for capability reporting.
pub const KNOWN_COMMANDS: &[&str] = &[
    "init",
    "status",
    "capabilities",
    "record-import",
    "evidence-import",
    "evidence-forget",
    "query",
    "packet",
    "gate",
    "verify",
    "export",
    "check",
    "review",
    "decision",
    "logo",
    "logo-inspect",
    "logo-proof",
    "logo-review-packet",
    "color",
    "color-check",
    "ingest",
    "test",
    "patch",
    "review-packet",
];

/// Check if a command is known.
pub fn is_known_command(cmd: &str) -> bool {
    KNOWN_COMMANDS.contains(&cmd) || matches!(cmd, "help" | "version" | "protocol")
}

/// Validate that a brand-scoped command has --brand.
pub fn require_brand(args: &CliArgs) -> Result<&str, String> {
    args.brand
        .as_deref()
        .ok_or_else(|| format!("command '{}' requires --brand", args.command))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version() {
        let args = parse_args(&["--version".into()]).unwrap();
        assert_eq!(args.command, "version");
    }

    #[test]
    fn parse_help() {
        let args = parse_args(&["--help".into()]).unwrap();
        assert_eq!(args.command, "help");
    }

    #[test]
    fn parse_command_with_brand() {
        let args = parse_args(&["status".into(), "--brand".into(), "acme".into()]).unwrap();
        assert_eq!(args.command, "status");
        assert_eq!(args.brand.as_deref(), Some("acme"));
    }

    #[test]
    fn parse_invalid_brand_rejected() {
        let result = parse_args(&["status".into(), "--brand".into(), "INVALID".into()]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_format() {
        let args = parse_args(&["query".into(), "--format".into(), "json".into()]).unwrap();
        assert_eq!(args.format, OutputFormat::Json);
    }

    #[test]
    fn parse_max_bytes_minimum() {
        let result = parse_args(&["query".into(), "--max-bytes".into(), "100".into()]);
        assert!(result.is_err());
    }

    #[test]
    fn empty_args_defaults_to_help() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(args.command, "help");
    }

    #[test]
    fn known_commands() {
        assert!(is_known_command("init"));
        assert!(is_known_command("status"));
        assert!(is_known_command("protocol"));
        assert!(!is_known_command("bogus"));
    }

    #[test]
    fn require_brand_works() {
        let args = parse_args(&["status".into(), "--brand".into(), "acme".into()]).unwrap();
        assert!(require_brand(&args).is_ok());

        let args2 = parse_args(&["status".into()]).unwrap();
        assert!(require_brand(&args2).is_err());
    }
}
