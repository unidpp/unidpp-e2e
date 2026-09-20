//! The story's service layer: the registry, resolver, gateway, quorum
//! trust, live trust and log helpers, the three issuer verbs (the
//! issuer-hook port), and the asserting verify runner. Every narration
//! string is the script's, verbatim.

use crate::engine::{Config, Sink, Story, R};
use crate::http::{self, urlencode};
use crate::json;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::time::Duration;

const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

impl Story {
    // -- URL plumbing -----------------------------------------------------

    fn registry_target(&self) -> (String, u16) {
        match &self.cfg.registry_url_override {
            Some(url) => Config::split_url(url),
            None => Config::split_bind(&self.cfg.registry_bind),
        }
    }

    pub fn registry_url(&self) -> String {
        match &self.cfg.registry_url_override {
            Some(url) => url.clone(),
            None => format!("http://{}", self.cfg.registry_bind),
        }
    }

    pub fn gateway_url(&self) -> String {
        format!("http://{}", self.cfg.gateway_bind)
    }

    pub fn resolver_url(&self) -> String {
        format!("http://{}", self.cfg.resolver_bind)
    }

    pub fn quorum_url_public(&self) -> String {
        match &self.quorum_url_override {
            Some(url) => url.clone(),
            None => format!("http://{}", self.cfg.quorum_bind),
        }
    }

    fn http_get(&self, base: &str, path: &str) -> Option<http::HttpReply> {
        let (host, port) = Config::split_url(base);
        http::get(&host, port, path, HTTP_TIMEOUT)
    }

    fn http_post(&self, base: &str, path: &str, body: &str) -> Option<http::HttpReply> {
        let (host, port) = Config::split_url(base);
        http::post(&host, port, path, body, HTTP_TIMEOUT)
    }

    // -- Registry ---------------------------------------------------------

    /// POST with the script's tolerance: 4xx is fine for re-runs
    /// (idempotent registration answers 409 "already registered"); only
    /// server errors abort the story.
    pub fn registry_post(&mut self, path: &str, body: &str, out: &Path) -> R<()> {
        self.out
            .show(&format!("POST {}{path}  {body}", self.registry_url()));
        let reply = self
            .http_post(&self.registry_url(), path, body)
            .ok_or_else(|| format!("registry POST {path} unreachable"))
            .map_err(|m| self.abort(m))?;
        let _ = fs::write(out, &reply.body);
        if reply.status >= 500 {
            return Err(self.abort(format!(
                "registry POST {path} returned {} (see {})",
                reply.status, reply.body
            )));
        }
        Ok(())
    }

    pub fn registry_get(&mut self, path: &str, out: &Path) -> R<()> {
        self.out.show(&format!("GET {}{path}", self.registry_url()));
        let reply = self
            .http_get(&self.registry_url(), path)
            .ok_or_else(|| format!("registry GET failed: {path}"))
            .map_err(|m| self.abort(m))?;
        let _ = fs::write(out, &reply.body);
        Ok(())
    }

    /// Has the registry already seen this applicability triple? The
    /// registry does not deduplicate bindings, so the demo detects the
    /// duplicate client-side (silent query — no show line).
    pub fn binding_already_seen(&self, product_type: &str) -> usize {
        let path = format!(
            "/applicability?product_type={}&at=2028-06-01T00%3A00%3A00Z",
            urlencode(product_type)
        );
        let reply = self.http_get(&self.registry_url(), &path);
        let body = reply.map(|r| r.body).unwrap_or_default();
        let _ = fs::write(self.artifact("reg-applicability-check.json"), &body);
        serde_json::from_str::<Value>(&body)
            .ok()
            .map(|doc| json::count_array(&doc, "applicability"))
            .unwrap_or(0)
    }

    pub fn start_registry(&mut self) -> R<()> {
        if self.cfg.registry_url_override.is_some() {
            self.out.note(&format!(
                "using external registry at {} (not starting one)",
                self.registry_url()
            ));
        } else {
            if !self.cfg.registry_bin.is_file() {
                return Err(self.abort(format!(
                    "registry binary missing: {} (run: make deps)",
                    self.cfg.registry_bin.display()
                )));
            }
            self.out.note(&format!(
                "starting unidpp-registry on {}",
                self.cfg.registry_bind
            ));
            let bind = self.cfg.registry_bind.clone();
            self.registry = self.spawn_service(
                &self.cfg.registry_bin.clone(),
                &[("UNIDPP_REGISTRY_BIND", &bind)],
            );
        }
        let (host, port) = self.registry_target();
        if !self.wait_healthy(&host, port) {
            return Err(self.abort(format!(
                "registry did not become healthy on {}",
                self.registry_url()
            )));
        }
        self.out.say(&format!(
            "unidpp-registry healthy at {} (19135 item service, )",
            self.registry_url()
        ));
        Ok(())
    }

    // -- Resolver (G-CORR) -------------------------------------------------

    pub fn start_resolver(&mut self) -> R<()> {
        let (host, port) = Config::split_bind(&self.cfg.resolver_bind);
        if self.wait_healthy_probe(&host, port) {
            self.out.note(&format!(
                "using already-running unidpp-resolver at {}",
                self.resolver_url()
            ));
            return Ok(());
        }
        if !self.cfg.resolver_bin.is_file() {
            return Err(self.abort(format!(
                "resolver binary missing: {} (run: make deps)",
                self.cfg.resolver_bin.display()
            )));
        }
        self.out.note(&format!(
            "starting unidpp-resolver on {} (journal: resolver-journal.jsonl)",
            self.cfg.resolver_bind
        ));
        let bind = self.cfg.resolver_bind.clone();
        let state = self.artifact("resolver-journal.jsonl");
        let state = state.display().to_string();
        self.resolver = self.spawn_service(
            &self.cfg.resolver_bin.clone(),
            &[("UNIDPP_BIND", &bind), ("UNIDPP_STATE_FILE", &state)],
        );
        if !self.wait_healthy(&host, port) {
            return Err(self.abort(format!(
                "resolver did not become healthy on {}",
                self.resolver_url()
            )));
        }
        Ok(())
    }

    /// POST one of the resolver's admin bodies; the reply comes back
    /// whole (the G-CORR checks read the status and the body).
    pub fn resolver_post(&self, path: &str, body: &str) -> Option<http::HttpReply> {
        self.http_post(&self.resolver_url(), path, body)
    }

    /// A GET that keeps both the raw header block and the body (the
    /// side-1 resolve asserts a response header).
    pub fn resolve_with_headers(
        &self,
        identifier: &str,
        extra: &[(&str, &str)],
    ) -> Option<http::HttpReply> {
        let mut query = format!("identifier={}", urlencode(identifier));
        for (key, value) in extra {
            query.push_str(&format!("&{key}={}", urlencode(value)));
        }
        let path = format!("/resolve?{query}");
        self.http_get(&self.resolver_url(), &path)
    }

    // -- Quorum trust (B-QUORUM) -------------------------------------------

    /// The trust base for the quorum beat: the live trust service, or
    /// an ephemeral instance with a fresh journal. Err carries the
    /// narrated-skip reason (the beat never stages the cryptography).
    pub fn start_quorum_trust(&mut self) -> Result<(), String> {
        if let Some(trust_url) = self.cfg.trust_url.clone() {
            self.quorum_url_override = Some(trust_url.clone());
            self.out.note(&format!(
                "B-QUORUM targets the live trust service at {trust_url}"
            ));
            return Ok(());
        }
        if !self.cfg.trust_bin.is_file() {
            return Err(format!(
                "unidpp-trust binary missing ({}; run: make deps-trust)",
                self.cfg.trust_bin.display()
            ));
        }
        self.out.note(&format!(
            "starting ephemeral unidpp-trust on {} (B-QUORUM only)",
            self.cfg.quorum_bind
        ));
        // The ephemeral journal is this run's own artifact; a stale one
        // from a previous run would collide with the fresh sequencing.
        let _ = fs::remove_file(self.artifact("quorum-trust.journal.jsonl"));
        let bind = self.cfg.quorum_bind.clone();
        let state = self
            .artifact("quorum-trust.journal.jsonl")
            .display()
            .to_string();
        self.quorum = self.spawn_service(
            &self.cfg.trust_bin.clone(),
            &[
                ("UNIDPP_TRUST_BIND", &bind),
                ("UNIDPP_TRUST_STATE_FILE", &state),
                ("UNIDPP_TRUST_NO_SEED_FIXTURES", "1"),
            ],
        );
        let (host, port) = Config::split_bind(&self.cfg.quorum_bind);
        if !self.wait_healthy(&host, port) {
            Self::stop_process(&mut self.quorum);
            return Err("the ephemeral trust service did not become healthy".to_string());
        }
        Ok(())
    }

    /// POST a body file; the status code comes back, the response body
    /// lands in the out file.
    pub fn quorum_post(&mut self, path: &str, body_file: &Path, out: &Path) -> Option<u16> {
        let file_name = body_name(body_file);
        self.out.show(&format!(
            "POST {}{path}  ({file_name})",
            self.quorum_url_public()
        ));
        let body = fs::read_to_string(body_file).ok()?;
        let reply = self.http_post(&self.quorum_url_public(), path, &body)?;
        let _ = fs::write(out, &reply.body);
        Some(reply.status)
    }

    /// A dotted field of the first standing entry for the beat's
    /// subject; booleans print lowercase, an empty list prints "none".
    pub fn quorum_standing(&self, query_suffix: &str, dotted_field: &str) -> String {
        let path = format!(
            "/revocations?{query_suffix}&subject={}",
            urlencode("node:haichuan-cn")
        );
        let Some(reply) = self.http_get(&self.quorum_url_public(), &path) else {
            return String::new();
        };
        let Ok(doc) = serde_json::from_str::<Value>(&reply.body) else {
            return String::new();
        };
        let Some(first) = doc
            .get("revocations")
            .and_then(Value::as_array)
            .and_then(|list| list.first())
        else {
            return "none".to_string();
        };
        match json::dotted(first, dotted_field) {
            Some(Value::Bool(b)) => b.to_string(),
            Some(v) => json::py_print(v),
            None => String::new(),
        }
    }

    // -- Gateway (B-INT) ----------------------------------------------------

    pub fn start_gateway(&mut self) -> R<()> {
        let (host, port) = Config::split_bind(&self.cfg.gateway_bind);
        if self.wait_healthy_probe(&host, port) {
            self.out.note(&format!(
                "using already-running unidpp-gateway at {}",
                self.gateway_url()
            ));
            return Ok(());
        }
        if !self.cfg.gateway_bin.is_file() {
            return Err(self.abort(format!(
                "gateway binary missing: {} (run: make deps)",
                self.cfg.gateway_bin.display()
            )));
        }
        if self.issuer_mode() {
            self.out.note(&format!(
                "starting unidpp-gateway on {} (issuer upstream: {})",
                self.cfg.gateway_bind,
                self.cfg.issuer_url.clone().unwrap_or_default()
            ));
        } else {
            self.out.note(&format!(
                "starting unidpp-gateway on {} (seeded fixtures — no issuer upstream)",
                self.cfg.gateway_bind
            ));
        }
        let bind = self.cfg.gateway_bind.clone();
        self.gateway = self.spawn_service(
            &self.cfg.gateway_bin.clone(),
            &[("UNIDPP_GATEWAY_BIND", &bind)],
        );
        if !self.wait_healthy(&host, port) {
            return Err(self.abort(format!(
                "unidpp-gateway did not become healthy on {}",
                self.gateway_url()
            )));
        }
        self.out.say(&format!(
            "unidpp-gateway healthy at {} (UNTP triad render + ingest)",
            self.gateway_url()
        ));
        Ok(())
    }

    pub fn gateway_get(&mut self, path: &str, out: &Path) -> R<()> {
        self.out.show(&format!("GET {}{path}", self.gateway_url()));
        let reply = self
            .http_get(&self.gateway_url(), path)
            .ok_or_else(|| format!("gateway GET failed: {path}"))
            .map_err(|m| self.abort(m))?;
        let _ = fs::write(out, &reply.body);
        Ok(())
    }

    /// POST a body file to /untp/ingest: the status code comes back,
    /// the response body lands in the out file.
    pub fn gateway_ingest(&mut self, body_file: &Path, out: &Path) -> Option<u16> {
        let body = fs::read_to_string(body_file).ok()?;
        let reply = self.http_post(&self.gateway_url(), "/untp/ingest", &body)?;
        let _ = fs::write(out, &reply.body);
        Some(reply.status)
    }

    // -- Live trust and transparency log ------------------------------------

    /// When UNIDPP_TRUST_URL is set (and issuance runs through the
    /// issuer service), pin the public anchor a verifier uses from the
    /// trust service's /keyring — role sign-ecdsa-p256.
    pub fn pin_trust_anchor(&mut self) -> R<()> {
        let Some(trust_url) = self.cfg.trust_url.clone() else {
            return Ok(());
        };
        if !self.issuer_mode() {
            self.out
                .note("UNIDPP_TRUST_URL set but issuance is CLI-driven — the trust-pinned");
            self.out
                .note("anchor covers the issuer's pack signer; keeping the mint-returned anchors");
            return Ok(());
        }
        self.out.show(&format!("GET {trust_url}/keyring"));
        let Some(reply) = self.http_get(&trust_url, "/keyring") else {
            return Err(self.abort(format!("trust /keyring unreachable on {trust_url}")));
        };
        let _ = fs::write(self.artifact("trust-keyring.json"), &reply.body);
        let doc: Value = serde_json::from_str(&reply.body)
            .map_err(|e| self.abort(format!("invalid keyring body: {e}")))?;
        let anchor = json::dotted_print(&doc, "roles.sign-ecdsa-p256.public");
        self.trust_key_id = json::dotted_print(&doc, "roles.sign-ecdsa-p256.key_id");
        self.trust_mode = json::dotted_print(&doc, "mode");
        if anchor.is_empty() {
            return Err(
                self.abort("trust /keyring carried no sign-ecdsa-p256 public anchor".to_string())
            );
        }
        self.trust_anchor = Some(anchor.clone());
        self.out
            .say(&format!("trust:    anchor pinned from {trust_url}/keyring"));
        self.out.say(&format!(
            "          role sign-ecdsa-p256, key id {} ({})",
            self.trust_key_id, self.trust_mode
        ));
        self.out
            .say("          every verify below passes this pinned key as --anchor");
        Ok(())
    }

    /// Read the transparency-log identity once (UNIDPP_LOG_URL mode).
    pub fn probe_log(&mut self) -> R<()> {
        let Some(log_url) = self.cfg.log_url.clone() else {
            return Ok(());
        };
        self.out.show(&format!("GET {log_url}/tree/head"));
        let Some(reply) = self.http_get(&log_url, "/tree/head") else {
            return Err(self.abort(format!("log /tree/head unreachable on {log_url}")));
        };
        let _ = fs::write(self.artifact("log-head.json"), &reply.body);
        let doc: Value = serde_json::from_str(&reply.body)
            .map_err(|e| self.abort(format!("invalid tree head: {e}")))?;
        let log_id = json::field_print(&doc, "log_id");
        self.log_id = Some(log_id.clone());
        self.out.say(&format!(
            "log:      {log_url} (log id {log_id}) — every minted pack anchors here"
        ));
        Ok(())
    }

    /// Anchor a minted pack's commitment in the live transparency log:
    /// assert the receipt echoes our commitment and that GET
    /// /receipt/{seq} re-serves it byte-identically.
    pub fn log_anchor_pack(&mut self, pack_file: &Path, label: &str, subject: &str) -> R<()> {
        let Some(log_url) = self.cfg.log_url.clone() else {
            return Ok(());
        };
        let receipts = self.cfg.work_dir.join("log-receipts");
        let _ = fs::create_dir_all(&receipts);
        let hash = json::sha256_hex(pack_file).map_err(|e| self.abort(e))?;
        let pack_name = body_name(pack_file);
        self.out.show(&format!(
            "POST {log_url}/commitments  (subject {subject}, commitment = sha256 of {pack_name})"
        ));
        let request_body = format!("{{\"subject\":\"{subject}\",\"commitment\":\"{hash}\"}}");
        let request_path = receipts.join(format!("{label}.request.json"));
        let _ = fs::write(&request_path, &request_body);
        let Some(reply) = self.http_post(&log_url, "/commitments", &request_body) else {
            return Err(self.abort(format!("log refused the {label} pack commitment")));
        };
        let receipt_path = receipts.join(format!("{label}.receipt.json"));
        let _ = fs::write(&receipt_path, &reply.body);
        let _ = fs::remove_file(&request_path);
        let receipt: Value = serde_json::from_str(&reply.body)
            .map_err(|e| self.abort(format!("invalid {label} receipt: {e}")))?;
        let rid = json::field_print(&receipt, "receipt_id");
        let seq = json::field_print(&receipt, "seq");
        let size = json::dotted_print(&receipt, "tree_head.tree_size");
        self.check(
            &format!("log receipt commitment echoes pack hash ({label})"),
            &hash,
            &json::field_print(&receipt, "commitment"),
        );
        let reserved_path = receipts.join(format!("{label}.reserved.json"));
        let identical = self
            .http_get(&log_url, &format!("/receipt/{seq}"))
            .map(|served| {
                let _ = fs::write(&reserved_path, &served.body);
                served.body.as_bytes() == reply.body.as_bytes()
            })
            .unwrap_or(false);
        if fs::read(&reserved_path).is_ok() {
            let _ = fs::remove_file(&reserved_path);
        }
        self.check(
            &format!("log receipt {rid} re-served byte-identically"),
            "ok",
            if identical { "ok" } else { "changed" },
        );
        self.out.say(&format!(
            "log:      receipt {rid} (seq {seq}, tree size {size}) — GET {log_url}/receipt/{seq}"
        ));
        Ok(())
    }

    // -- The issuer verbs (the issuer-hook port) -----------------------------

    /// issuer_create <id> <type-ref|-> <capability> <eo> <resolver>
    ///                   <passport-urn> <out-file> [config]
    pub fn issuer_create(&mut self, spec: &CreateSpec) -> R<()> {
        if self.issuer_mode() {
            let issuer_url = self.cfg.issuer_url.clone().unwrap_or_default();
            self.out
                .note(&format!("issuer service mode: POST /passports {}", spec.id));
            let body = self.issuer_create_body(spec);
            let out = spec.out.clone();
            let reply = self.http_post_bearer(&issuer_url, "/passports", &body);
            match reply {
                Some(r) if r.status < 400 => {
                    let _ = fs::write(&out, &r.body);
                }
                _ => {
                    // 409 (already issued) against a stateful issuer
                    // journal: the identity is never re-minted (I1).
                    self.out
                        .note("already issued server-side — re-using the existing document");
                    let fetched = self
                        .issuer_get_passport_body(&issuer_url, &spec.urn)
                        .ok_or_else(|| {
                            self.abort(format!(
                                "issuer has no passport {} and refused the create",
                                spec.urn
                            ))
                        })?;
                    let _ = fs::write(&out, &fetched);
                }
            }
            // Sync the local mirror so the narration helpers read the
            // same document the service holds.
            let mirror = self
                .issuer_get_passport_body(&issuer_url, &spec.urn)
                .ok_or_else(|| self.abort(format!("issuer lost passport {}", spec.urn)))?;
            let _ = fs::write(&out, &mirror);
        } else {
            let mut argv = vec![
                self.cfg.unidpp.display().to_string(),
                "create".to_string(),
                "--id".to_string(),
                spec.id.clone(),
                "--capability".to_string(),
                spec.capability.clone(),
                "--eo".to_string(),
                spec.eo.clone(),
                "--passport-id".to_string(),
                spec.urn.clone(),
                "--out".to_string(),
                spec.out.display().to_string(),
            ];
            if spec.type_ref != "-" {
                argv.push("--type".to_string());
                argv.push(spec.type_ref.clone());
            }
            if spec.resolver != "-" {
                argv.push("--resolver".to_string());
                argv.push(spec.resolver.clone());
            }
            let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
            self.run_quiet(&refs)?;
        }
        Ok(())
    }

    fn issuer_create_body(&self, spec: &CreateSpec) -> String {
        let mut body = json!({
            "identity": spec.id,
            "capability": spec.capability,
        });
        let map = body.as_object_mut().expect("body is an object");
        if spec.type_ref != "-" {
            map.insert("type_ref".to_string(), json!(spec.type_ref));
        }
        if spec.eo != "-" {
            map.insert("eo_id".to_string(), json!(spec.eo));
        }
        if spec.resolver != "-" {
            map.insert("resolver_uri".to_string(), json!(spec.resolver));
        }
        if spec.urn != "-" {
            map.insert("passport_id".to_string(), json!(spec.urn));
        }
        if let Some(config) = &spec.config {
            let vector: Vec<String> = config
                .split(',')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .collect();
            map.insert("config".to_string(), json!(vector));
        }
        body.to_string()
    }

    /// GET the full passport view (the mirror-sync verb of the hook).
    pub fn issuer_get_passport(&mut self, urn: &str) -> Option<String> {
        let issuer_url = self.cfg.issuer_url.clone().unwrap_or_default();
        self.issuer_get_passport_body(&issuer_url, urn)
    }

    fn issuer_get_passport_body(&self, issuer_url: &str, urn: &str) -> Option<String> {
        self.http_get_bearer(issuer_url, &format!("/passports/{}", urlencode(urn)))
            .filter(|r| r.status < 400)
            .map(|r| r.body)
    }

    /// issuer_event <passport-file> <event-type> <data-json> [actor] [role] [at]
    pub fn issuer_event(
        &mut self,
        passport_file: &Path,
        event_type: &str,
        data_json: &str,
        actor: &str,
        role: &str,
        at: &str,
    ) -> R<()> {
        if self.issuer_mode() {
            let issuer_url = self.cfg.issuer_url.clone().unwrap_or_default();
            let doc = json::load(passport_file).map_err(|e| self.abort(e))?;
            let urn = json::field_print(&doc, "passport_id");
            self.out.note(&format!(
                "issuer service mode: POST /passports/{urn}/events ({event_type})"
            ));
            let body = issuer_event_body(event_type, data_json, actor, role, at);
            let temp = self.artifact(&format!("issuer-event-{}.json", std::process::id()));
            if let Some(r) = self.http_post_bearer(
                &issuer_url,
                &format!("/passports/{}/events", urlencode(&urn)),
                &body,
            ) {
                let _ = fs::write(&temp, &r.body);
            }
            let mirror = self
                .issuer_get_passport_body(&issuer_url, &urn)
                .ok_or_else(|| self.abort(format!("issuer lost passport {urn}")))?;
            let _ = fs::write(passport_file, &mirror);
        } else {
            let mut argv = vec![
                self.cfg.unidpp.display().to_string(),
                "event".to_string(),
                "--passport".to_string(),
                passport_file.display().to_string(),
                "--type".to_string(),
                event_type.to_string(),
                "--data".to_string(),
                data_json.to_string(),
            ];
            if !actor.is_empty() {
                argv.push("--actor".to_string());
                argv.push(actor.to_string());
            }
            if !role.is_empty() {
                argv.push("--actor-role".to_string());
                argv.push(role.to_string());
            }
            if !at.is_empty() {
                argv.push("--at".to_string());
                argv.push(at.to_string());
            }
            let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
            self.run_quiet(&refs)?;
        }
        Ok(())
    }

    /// issuer_mint_pack: mints the offline Tier-A pack and returns the
    /// signing anchor (public key, hex).
    pub fn issuer_mint_pack(&mut self, passport_file: &Path, out: &Path) -> R<String> {
        if self.issuer_mode() {
            let issuer_url = self.cfg.issuer_url.clone().unwrap_or_default();
            let doc = json::load(passport_file).map_err(|e| self.abort(e))?;
            let urn = json::field_print(&doc, "passport_id");
            self.out
                .note(&format!("issuer service mode: POST /passports/{urn}/pack"));
            let temp = self.artifact(&format!("issuer-pack-{}.json", std::process::id()));
            let reply = self
                .http_post_bearer(
                    &issuer_url,
                    &format!("/passports/{}/pack", urlencode(&urn)),
                    "{}",
                )
                .ok_or_else(|| {
                    self.abort(format!(
                        "pack minting failed for {}",
                        passport_file.display()
                    ))
                })?;
            let _ = fs::write(&temp, &reply.body);
            let pack_doc: Value = serde_json::from_str(&reply.body)
                .map_err(|e| self.abort(format!("invalid pack reply: {e}")))?;
            let pack = pack_doc.get("pack").cloned().unwrap_or(Value::Null);
            let _ = fs::write(out, json::py_dumps(&pack));
            let anchor = json::field_print(&pack_doc, "anchor");
            let _ = fs::remove_file(&temp);
            Ok(anchor)
        } else {
            let argv = [
                self.cfg.unidpp.display().to_string(),
                "pack".to_string(),
                "--passport".to_string(),
                passport_file.display().to_string(),
                "--key".to_string(),
                "e8-demo-pack-seed".to_string(),
                "--out".to_string(),
                out.display().to_string(),
            ];
            let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
            self.out.show(&argv.join(" "));
            let stderr_path = self
                .cfg
                .work_dir
                .join(format!("pack.stderr.{}", std::process::id()));
            let code = self.run_command(&refs, Sink::Null, Sink::File(stderr_path.clone()));
            if code != Some(0) {
                return Err(self.abort(format!(
                    "pack minting failed for {}",
                    passport_file.display()
                )));
            }
            let log_text = fs::read_to_string(&stderr_path).unwrap_or_default();
            let _ = fs::remove_file(&stderr_path);
            let anchor = log_text
                .lines()
                .find_map(|l| {
                    l.split_once("anchor (public key, hex): ")
                        .map(|(_, a)| a.trim())
                })
                .unwrap_or_default()
                .to_string();
            Ok(anchor)
        }
    }

    fn http_post_bearer(&self, base: &str, path: &str, body: &str) -> Option<http::HttpReply> {
        let (host, port) = Config::split_url(base);
        http::request(
            &host,
            port,
            "POST",
            path,
            Some(body),
            HTTP_TIMEOUT,
            self.cfg.issuer_admin_token.as_deref(),
        )
    }

    fn http_get_bearer(&self, base: &str, path: &str) -> Option<http::HttpReply> {
        let (host, port) = Config::split_url(base);
        http::request(
            &host,
            port,
            "GET",
            path,
            None,
            HTTP_TIMEOUT,
            self.cfg.issuer_admin_token.as_deref(),
        )
    }

    // -- The asserting verify runner ------------------------------------------

    /// Run `unidpp verify` and ASSERT the expected verdict: 0 pass,
    /// 1 degraded, 2 fail — an unexpected outcome aborts the demo.
    pub fn verify_and_expect(
        &mut self,
        pack: &Path,
        anchor: &str,
        as_of: &str,
        expected: i32,
        why: &str,
        extra: &[&str],
    ) -> R<()> {
        // Live trust mode: the anchor is the one pinned from the trust
        // service's /keyring; the per-pack argument stays as the
        // CLI-driver fallback.
        let anchor = self
            .trust_anchor
            .clone()
            .unwrap_or_else(|| anchor.to_string());
        let label = match expected {
            0 => "pass",
            1 => "degraded",
            2 => "fail",
            other => {
                return Err(self.abort(format!("internal: bad expected verdict {other}")));
            }
        };
        self.out.show(&format!(
            "unidpp verify {} --anchor <pinned> --as-of {as_of} {}   # expect: {label}",
            pack.display(),
            extra.join(" ")
        ));
        let mut argv = vec![
            self.cfg.unidpp.display().to_string(),
            "verify".to_string(),
            pack.display().to_string(),
            "--anchor".to_string(),
            anchor,
            "--as-of".to_string(),
            as_of.to_string(),
        ];
        argv.extend(extra.iter().map(|s| s.to_string()));
        let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        let code = self.run_command(&refs, Sink::Tee, Sink::Tee);
        let Some(code) = code else {
            return Err(self.abort(format!(
                "unidpp CLI not found at {} (run: make deps)",
                self.cfg.unidpp.display()
            )));
        };
        if code == 127 {
            return Err(self.abort(format!(
                "unidpp CLI not found at {} (run: make deps)",
                self.cfg.unidpp.display()
            )));
        }
        self.check(
            &format!("verify verdict ({why})"),
            &expected.to_string(),
            &code.to_string(),
        );
        if code != expected {
            return Err(self.abort(format!(
                "verify returned {code}, expected {expected} ({label}): {why}"
            )));
        }
        Ok(())
    }
}

pub struct CreateSpec {
    pub id: String,
    pub type_ref: String,
    pub capability: String,
    pub eo: String,
    pub resolver: String,
    pub urn: String,
    pub out: std::path::PathBuf,
    pub config: Option<String>,
}

/// The externally-tagged wrapper the issuer's payload_from_data expects.
fn issuer_event_body(
    event_type: &str,
    data_json: &str,
    actor: &str,
    role: &str,
    at: &str,
) -> String {
    let payload: Value = serde_json::from_str(data_json).unwrap_or(Value::Null);
    let wrapped = match event_type {
        "issuance" => json!({ "Issuance": payload }),
        "custody.transfer" => json!({ "CustodyTransfer": payload }),
        "split" => json!({ "Split": payload }),
        "combine" => json!({ "Combine": payload }),
        "end-of-waste" => json!({ "EndOfWaste": payload }),
        "decompose" => json!({ "Decompose": payload }),
        "install" => json!({ "Install": payload }),
        "uninstall" => json!({ "Uninstall": payload }),
        "part.replace" => json!({ "PartReplace": payload }),
        "consumable.replace" => json!({ "ConsumableReplace": payload }),
        "repair.perform" => json!({ "RepairPerform": payload }),
        "product.modify" => json!({ "ProductModify": payload }),
        "software.update" => json!({ "SoftwareUpdate": payload }),
        "refurbish.remanufacture" => json!({ "RefurbishRemanufacture": payload }),
        "recall.campaign" => json!({ "RecallCampaign": payload }),
        "correction" => json!({ "Correction": payload }),
        "status.change" => json!({ "StatusChange": payload }),
        "flag.security" => json!({ "FlagSecurity": payload }),
        "inspection.stamp" => json!({ "InspectionStamp": payload }),
        "milestone.record" => json!({ "MilestoneRecord": payload }),
        _ => payload,
    };
    let mut body = json!({ "type": event_type, "data": wrapped });
    let map = body.as_object_mut().expect("body is an object");
    if !actor.is_empty() {
        map.insert("actor".to_string(), json!(actor));
    }
    if !role.is_empty() {
        map.insert("actor_role".to_string(), json!(role));
    }
    if !at.is_empty() {
        map.insert("at".to_string(), json!(at));
    }
    body.to_string()
}

/// basename(1) for the show lines that name a body file.
fn body_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
