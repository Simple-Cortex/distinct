//! `name check`: offline red-flag screen for naming candidates.
//!
//! Reports NFKC changes, Unicode scripts, UTS #39 confusable skeletons and
//! flag-list hits. It makes no network calls, issues no verdict or score, and
//! is never trademark clearance or linguistic screening.

use crate::dispatch::{self, write_envelope, write_error, CliArgs};
use distinct_core::Exit;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Write;
use unicode_normalization::UnicodeNormalization;
use unicode_script::{Script, UnicodeScript};
use unicode_security::confusable_detection::skeleton;
use unicode_security::MixedScript;

const DEFAULT_FLAGS: &str = include_str!("../data/flag-terms-default.txt");
const MAX_CANDIDATES: usize = 200;
const MAX_CANDIDATE_CHARS: usize = 200;
const MAX_INPUT_BYTES: u64 = 1 << 20;

const LIMITATIONS: [&str; 4] = [
    "offline_heuristic_only",
    "not_trademark_clearance",
    "not_linguistic_screening",
    "flag_list_incomplete",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    candidates: Vec<String>,
    #[serde(default = "default_lists")]
    flag_lists: Vec<String>,
}

fn default_lists() -> Vec<String> {
    vec!["default".to_string()]
}

#[derive(Serialize)]
struct FlagHit {
    list: String,
    term: String,
    #[serde(rename = "match")]
    kind: &'static str,
}

#[derive(Serialize)]
struct CandidateReport {
    candidate: String,
    normalized: String,
    changed_by_nfkc: bool,
    scripts: Vec<String>,
    mixed_script: bool,
    confusable_skeleton: String,
    confusable_with: Vec<String>,
    non_ascii: bool,
    length_chars: usize,
    flag_hits: Vec<FlagHit>,
}

/// Lowercased words of a name, split on non-alphanumerics and on
/// lower-to-upper camelCase boundaries ("GrapeAnalytics" -> grape, analytics).
fn words_of(s: &str) -> BTreeSet<String> {
    let mut words = BTreeSet::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in s.chars() {
        if (!c.is_alphanumeric() || (prev_lower && c.is_uppercase())) && !cur.is_empty() {
            words.insert(std::mem::take(&mut cur).to_lowercase());
        }
        if c.is_alphanumeric() {
            cur.push(c);
        }
        prev_lower = c.is_lowercase();
    }
    if !cur.is_empty() {
        words.insert(cur.to_lowercase());
    }
    words
}

/// A parsed flag term: `exact_only` (`=`-prefixed) terms match a whole word
/// or the whole name, never a substring inside a longer word.
struct Term {
    text: String,
    exact_only: bool,
}

fn parse_terms(src: &str) -> Vec<Term> {
    src.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| match l.strip_prefix('=') {
            Some(rest) => Term {
                text: rest.to_string(),
                exact_only: true,
            },
            None => Term {
                text: l.to_string(),
                exact_only: false,
            },
        })
        .collect()
}

fn flag_list(name: &str) -> Option<Vec<Term>> {
    match name {
        "default" => Some(parse_terms(DEFAULT_FLAGS)),
        _ => None,
    }
}

fn scripts_of(s: &str) -> Vec<String> {
    let set: BTreeSet<&'static str> = s
        .chars()
        .map(|c| c.script())
        .filter(|sc| !matches!(sc, Script::Common | Script::Inherited | Script::Unknown))
        .map(|sc| sc.full_name())
        .collect();
    set.into_iter().map(str::to_string).collect()
}

fn validate(req: &Request) -> Result<(), String> {
    if req.candidates.is_empty() {
        return Err("candidates must not be empty".into());
    }
    if req.candidates.len() > MAX_CANDIDATES {
        return Err(format!("at most {MAX_CANDIDATES} candidates"));
    }
    for c in &req.candidates {
        if c.trim().is_empty() {
            return Err("candidates must be non-empty strings".into());
        }
        if c.chars().count() > MAX_CANDIDATE_CHARS {
            return Err(format!("candidate longer than {MAX_CANDIDATE_CHARS} chars"));
        }
    }
    Ok(())
}

fn analyse(req: &Request) -> Result<Vec<CandidateReport>, String> {
    let mut lists = Vec::new();
    for name in &req.flag_lists {
        let terms = flag_list(name).ok_or_else(|| format!("unknown flag list '{name}'"))?;
        lists.push((name.as_str(), terms));
    }

    let normalized: Vec<String> = req.candidates.iter().map(|c| c.nfkc().collect()).collect();
    let skeletons: Vec<String> = normalized.iter().map(|n| skeleton(n).collect()).collect();

    let reports = req
        .candidates
        .iter()
        .enumerate()
        .map(|(i, cand)| {
            let norm = &normalized[i];
            let lower = norm.to_lowercase();
            let words = words_of(norm);
            let scripts = scripts_of(norm);
            let confusable_with = req
                .candidates
                .iter()
                .enumerate()
                .filter(|(j, other)| *j != i && skeletons[*j] == skeletons[i] && *other != cand)
                .map(|(_, other)| other.clone())
                .collect();
            let mut flag_hits = Vec::new();
            for (list, terms) in &lists {
                for t in terms {
                    let kind = if lower == t.text {
                        Some("exact")
                    } else if words.contains(&t.text) {
                        Some("word")
                    } else if !t.exact_only && lower.contains(&t.text) {
                        Some("substring")
                    } else {
                        None
                    };
                    if let Some(kind) = kind {
                        flag_hits.push(FlagHit {
                            list: (*list).to_string(),
                            term: t.text.clone(),
                            kind,
                        });
                    }
                }
            }
            CandidateReport {
                candidate: cand.clone(),
                changed_by_nfkc: norm != cand,
                normalized: norm.clone(),
                // UTS #39 augmented script sets: Han+Kana (Jpan) and
                // Hangul+Han (Kore) are single-script, Latin+Cyrillic is not.
                mixed_script: !norm.as_str().is_single_script(),
                scripts,
                confusable_skeleton: skeletons[i].clone(),
                confusable_with,
                non_ascii: !cand.is_ascii(),
                length_chars: norm.chars().count(),
                flag_hits,
            }
        })
        .collect();
    Ok(reports)
}

pub(crate) fn cmd_name<O: Write>(args: &CliArgs, out: &mut O) -> Exit {
    match args.rest.first().map(String::as_str) {
        Some("check") => check(args, out),
        _ => write_error(
            out,
            "name",
            "usage: name check --candidates <file.json>",
            Exit::Invalid,
        ),
    }
}

fn check<O: Write>(args: &CliArgs, out: &mut O) -> Exit {
    const CMD: &str = "name-check";
    if let Err(msg) = dispatch::require_brand(args) {
        return write_error(out, CMD, &msg, Exit::Invalid);
    }
    let Some(path) = args
        .rest
        .iter()
        .position(|a| a == "--candidates")
        .and_then(|i| args.rest.get(i + 1))
    else {
        return write_error(out, CMD, "--candidates <file.json> required", Exit::Invalid);
    };

    let text = match std::fs::metadata(path) {
        Ok(m) if m.len() > MAX_INPUT_BYTES => {
            return write_error(out, CMD, "candidates file too large", Exit::Invalid)
        }
        Ok(_) => match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                return write_error(out, CMD, &format!("cannot read {path}: {e}"), Exit::Invalid)
            }
        },
        Err(e) => return write_error(out, CMD, &format!("cannot read {path}: {e}"), Exit::Invalid),
    };
    let req: Request = match serde_json::from_str(&text) {
        Ok(r) => r,
        Err(e) => return write_error(out, CMD, &format!("invalid JSON: {e}"), Exit::Invalid),
    };
    if let Err(msg) = validate(&req) {
        return write_error(out, CMD, &msg, Exit::Invalid);
    }
    let reports = match analyse(&req) {
        Ok(r) => r,
        Err(msg) => return write_error(out, CMD, &msg, Exit::Invalid),
    };

    let limitations: Vec<String> = LIMITATIONS.iter().map(|s| (*s).to_string()).collect();
    let result = serde_json::json!({
        "flag_lists": req.flag_lists,
        "candidates": reports,
        "limitations": LIMITATIONS,
    });
    write_envelope(out, CMD, result, limitations, vec![])
}
