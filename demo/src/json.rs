//! Typed JSON assertions over serde_json — the Rust replacements for the
//! script's python3 one-liners. The renderings mirror Python's so the
//! narration lines are byte-identical to the shell transcript.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn load(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("invalid JSON in {}: {e}", path.display()))
}

/// Python's `print(doc[field])`: strings bare, numbers bare, booleans
/// capitalized, containers as json.dumps.
pub fn py_print(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => (if *b { "True" } else { "False" }).to_string(),
        Value::Null => "None".to_string(),
        other => py_dumps(other),
    }
}

/// Python's `json.dumps` (default separators, ensure_ascii).
pub fn py_dumps(v: &Value) -> String {
    match v {
        Value::String(s) => quote(s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(py_dumps).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Object(map) => {
            let parts: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", quote(k), py_dumps(v)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The script's `json_get`: a top-level field, printed the Python way.
pub fn field_print(doc: &Value, field: &str) -> String {
    doc.get(field).map(py_print).unwrap_or_default()
}

/// A dotted path with one nesting level per dot (the script's json_path).
pub fn dotted<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |acc, part| acc.get(part))
}

pub fn dotted_print(v: &Value, path: &str) -> String {
    dotted(v, path).map(py_print).unwrap_or_default()
}

/// The script's python3_count: the length of a top-level array field.
pub fn count_array(doc: &Value, field: &str) -> usize {
    doc.get(field).and_then(Value::as_array).map_or(0, Vec::len)
}

/// The installation edges of a passport document, in seal order —
/// payload.Install.target.Open, the projection the CTO checks traverse.
fn install_edges(doc: &Value) -> Vec<Value> {
    let empty: Vec<Value> = Vec::new();
    let sealed = doc
        .get("log")
        .and_then(|l| l.get("sealed"))
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    sealed
        .iter()
        .filter(|e| {
            e.get("event")
                .and_then(|ev| ev.get("event_type"))
                .and_then(Value::as_str)
                == Some("install")
        })
        .filter_map(|e| {
            e.get("event")?
                .get("payload")?
                .get("Install")?
                .get("target")?
                .get("Open")
                .cloned()
        })
        .collect()
}

/// How many installation edges point out of the document (CTO check).
pub fn outgoing_install_count(doc: &Value) -> usize {
    install_edges(doc)
        .iter()
        .filter(|edge| edge.get("direction").and_then(Value::as_str) == Some("outgoing"))
        .count()
}

/// The `other` passport of the n-th installation edge, or "" (the script
/// prints the empty string when the index is out of range).
pub fn install_child(doc: &Value, index: usize) -> String {
    install_edges(doc)
        .get(index)
        .and_then(|edge| edge.get("other"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// The UNTP identifier value a rendered triad carries for its subject.
pub fn triad_identifier(triad: &Value) -> String {
    triad
        .get("passport")
        .and_then(|p| p.get("productIdentifiers"))
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|id| id.get("value"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Did every conformity standard the triad rendered land as a profile
/// binding on the ingested passport? Same verdict strings as the script.
pub fn untp_bindings(triad: &Value, ingest: &Value) -> String {
    let standards: Vec<String> = triad
        .get("passport")
        .and_then(|p| p.get("standardsConformance"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|c| {
                    c.get("standard")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                })
                .collect()
        })
        .unwrap_or_default();
    let bound: Vec<String> = ingest
        .get("profiles")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|p| {
                    p.get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                })
                .collect()
        })
        .unwrap_or_default();
    if standards.is_empty() {
        "no standardsConformance rendered".to_string()
    } else if standards.iter().any(|s| !bound.contains(s)) {
        let missing: Vec<String> = standards
            .iter()
            .filter(|s| !bound.contains(s))
            .cloned()
            .collect();
        format!("unbound: {}", missing.join(","))
    } else {
        "ok".to_string()
    }
}

/// The counters of the last milestone.record event (the B5 narration).
pub fn last_milestone_counters(doc: &Value) -> Vec<(String, String)> {
    let sealed = doc.get("log").and_then(|l| l.get("sealed"));
    let last = sealed
        .and_then(Value::as_array)
        .and_then(|events| {
            events.iter().rev().find(|e| {
                e.get("event")
                    .and_then(|ev| ev.get("event_type"))
                    .and_then(Value::as_str)
                    == Some("milestone.record")
            })
        })
        .map(|e| {
            e.get("event")
                .and_then(|ev| ev.get("payload"))
                .unwrap_or(&Value::Null)
        });
    let counters = last
        .and_then(|p| {
            p.get("MilestoneRecord")
                .or(Some(p))
                .and_then(|m| m.get("counters"))
        })
        .and_then(Value::as_object);
    match counters {
        None => Vec::new(),
        Some(map) => map.iter().map(|(k, v)| (k.clone(), py_print(v))).collect(),
    }
}

/// The outputs of the last decompose event as (amount, child) pairs —
/// the B10 mass balance reads them from the artifact.
pub fn last_decompose_outputs(doc: &Value) -> Vec<(String, String)> {
    let sealed = doc.get("log").and_then(|l| l.get("sealed"));
    let last = sealed
        .and_then(Value::as_array)
        .and_then(|events| {
            events.iter().rev().find(|e| {
                e.get("event")
                    .and_then(|ev| ev.get("event_type"))
                    .and_then(Value::as_str)
                    == Some("decompose")
            })
        })
        .map(|e| {
            e.get("event")
                .and_then(|ev| ev.get("payload"))
                .unwrap_or(&Value::Null)
        });
    let outputs = last
        .and_then(|p| {
            p.get("Decompose")
                .or(Some(p))
                .and_then(|d| d.get("outputs"))
        })
        .and_then(Value::as_array);
    match outputs {
        None => Vec::new(),
        Some(list) => list
            .iter()
            .map(|o| {
                (
                    o.get("quantity")
                        .and_then(|q| q.get("amount"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    o.get("child")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                )
            })
            .collect(),
    }
}

/// The publish-only keyring reader: under `roles` (or the document
/// itself when the keyring ships bare), the `pack` role's public anchor
/// — the first of `public`, `public_hex`, `anchor` that is a string,
/// whatever shape the role entry takes (map or single-entry list).
pub fn keyring_pack_anchor(doc: &Value) -> Option<String> {
    let roles = doc.get("roles").unwrap_or(doc);
    let pack = roles.get("pack")?;
    let entry = match pack {
        Value::Object(_) => Some(pack),
        Value::Array(list) => list.first(),
        _ => None,
    }?;
    ["public", "public_hex", "anchor"]
        .iter()
        .find_map(|key| entry.get(key).and_then(Value::as_str))
        .map(str::to_string)
}

/// The EN 18222 binding's identity field (the gateway's AD-3 parity).
pub fn en18222_identity(doc: &Value) -> String {
    field_print(doc, "uniqueProductIdentifier")
}

/// Every product identifier value of a rendered UNTP triad.
pub fn untp_identifier_values(triad: &Value) -> Vec<String> {
    triad
        .get("passport")
        .and_then(|p| p.get("productIdentifiers"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|id| id.get("value").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The first identifier value of a rendered UNTP triad ("" when absent).
pub fn untp_first_identifier(triad: &Value) -> String {
    untp_identifier_values(triad)
        .first()
        .cloned()
        .unwrap_or_default()
}

/// What follows the last `)` (the `(01)<gtin>` GS1 spelling), or the
/// value itself when it carries no parenthesis.
pub fn after_last_paren(value: &str) -> &str {
    match value.rsplit_once(')') {
        Some((_, bare)) => bare,
        None => value,
    }
}

/// SHA-256 of a file's exact bytes, hex — the pack commitment.
pub fn sha256_hex(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let digest = Sha256::digest(&bytes);
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}
