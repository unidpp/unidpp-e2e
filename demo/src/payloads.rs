//! The typed JSON payload builders — verbatim the script's printf
//! templates (the wire shapes of unidpp-core/crates/event/src/payload.rs).

/// install_data <direction> <other-urn> <from> <alteration|-> <method>
///               <recoverability> <pairing>
pub fn install_data(
    direction: &str,
    other: &str,
    from: &str,
    alteration: &str,
    method: &str,
    recoverability: &str,
    pairing: &str,
) -> String {
    let alterations = if alteration == "-" {
        "[]".to_string()
    } else {
        format!("[\"Known\",\"{alteration}\"]")
    };
    format!(
        "{{\"target\":{{\"Open\":{{\"link_type\":\"installation\",\"other\":\"{other}\",\"direction\":\"{direction}\",\"interval\":{{\"from\":\"{from}\",\"to\":null}},\"binding\":{{\"method\":\"{method}\",\"recoverability\":\"{recoverability}\",\"visibility\":{{\"edge\":\"public\",\"audiences\":[]}},\"slot_id\":null,\"pairing\":\"{pairing}\",\"alterations\":{alterations}}}}}}}}}"
    )
}

/// uninstall_data <other-urn> <interval-from> <interval-to> — closes the
/// installation interval; the outcome is harvested (provenance carries
/// forward).
pub fn uninstall_data(other: &str, from: &str, to: &str) -> String {
    format!(
        "{{\"link\":{{\"link_type\":\"installation\",\"other\":\"{other}\",\"direction\":\"outgoing\",\"interval\":{{\"from\":\"{from}\",\"to\":\"{to}\"}},\"binding\":{{\"method\":\"fastened\",\"recoverability\":\"restorable\",\"visibility\":{{\"edge\":\"public\",\"audiences\":[]}},\"slot_id\":null,\"pairing\":\"firmware\",\"alterations\":[]}}}},\"outcome\":\"harvested\"}}"
    )
}

/// stamp_data <subject-urn> <attester> <at> — a lens-scoped, dated,
/// signed condition stamp.
pub fn stamp_data(subject: &str, attester: &str, at: &str) -> String {
    format!(
        "{{\"stamp\":{{\"attester\":\"{attester}\",\"subject\":\"{subject}\",\"subject_state_commitment\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"lens\":\"urn:unidpp:profile:lens-auction\",\"lens_version\":\"1\",\"mode\":\"snapshot\",\"verdict_summary\":\"grade A-\",\"coverage_report\":null,\"log_anchored_at\":\"{at}\",\"quantity_context\":null}}}}"
    )
}

/// decompose_data — B10 mass balance: 25.9 kg in; 21.4 + 4.3 out.
pub fn decompose_data(scrap_urn: &str, recycle_urn: &str) -> String {
    format!(
        "{{\"outputs\":[{{\"child\":\"{scrap_urn}\",\"quantity\":{{\"amount\":\"21.4\",\"unit\":{{\"uom\":\"kg\",\"registry_uri\":\"https://unitsml.org/units/kg\"}}}}}},{{\"child\":\"{recycle_urn}\",\"quantity\":{{\"amount\":\"4.3\",\"unit\":{{\"uom\":\"kg\",\"registry_uri\":\"https://unitsml.org/units/kg\"}}}}}}],\"accredited_for_claims\":true}}"
    )
}

/// Registry seed bodies.
pub fn reg_transform_body() -> String {
    "{\"register_id\":\"unidpp-e2e\",\"item_id\":\"gb4943-1-2022-eq-iec-62368-1\",\"class\":\"transform\",\"definition\":\"GB 4943.1-2022 ~= IEC 62368-1 certificate equivalence (attester: cqc)\",\"version\":\"1.0.0\"}".to_string()
}

pub fn reg_profile_body() -> String {
    "{\"register_id\":\"unidpp-e2e\",\"item_id\":\"eu-battery-lmt\",\"class\":\"profile\",\"definition\":\"EU battery passport profile - LMT class (Reg. (EU) 2023/1542)\",\"version\":\"1.0.0\",\"effective_from\":\"2027-02-18T00:00:00Z\",\"manifest\":{\"version\":\"1.0.0\",\"issuer_class\":\"law\",\"issuer\":\"ec-espr\",\"signature\":{\"signature\":\"seeded-dev-signature\"}}}".to_string()
}

pub fn reg_binding_body(product_type: &str) -> String {
    format!("{{\"profile_id\":\"eu-battery-lmt\",\"product_type\":\"{product_type}\",\"effective_from\":\"2028-02-01T00:00:00Z\"}}")
}
