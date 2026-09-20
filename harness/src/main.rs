//! The family harness entry point: the ordered leg table (the shell
//! harness's twelve tests, in its order), the per-leg `== test N ==`
//! headers, and the closing summary whose counts the ledger compares.
#![allow(clippy::zombie_processes)] // the bench projector and quickstart services are stopped on every path

mod engine;
mod http;
mod legs;
mod pattern;

use engine::Harness;
use std::fs;

/// One leg of the family CI: the number the shell printed, its label,
/// and the function that runs it.
struct Leg {
    number: u8,
    label: &'static str,
    run: fn(&mut Harness),
}

/// The legs in the shell harness's order: the demo story, the CLI
/// tamper and missing-anchor probes, the registry's dated binding, the
/// live B1-B4 subset, the three adoption-path quickstarts, the
/// service-level tenant isolation, the NF-1 bench, the FW-3 class
/// claims, and the hub relay.
const LEGS: &[Leg] = &[
    Leg {
        number: 1,
        label: "happy path: unidpp-demo walks B1-B10 successfully",
        run: legs::happy_path,
    },
    Leg {
        number: 2,
        label: "tamper detection: byte-flipped pack fails verification",
        run: legs::tamper_detection,
    },
    Leg {
        number: 3,
        label: "missing anchor: verifier degrades (exit 1), never PASS",
        run: legs::missing_anchor,
    },
    Leg {
        number: 4,
        label: "registry: EU lens binds after 2028-02-01 only",
        run: legs::registry_binding,
    },
    Leg {
        number: 5,
        label: "live services: B1-B4 subset over registry + issuer + trust + log",
        run: legs::live_services,
    },
    Leg {
        number: 6,
        label: "adoption path: verify-only (one binary, zero services)",
        run: legs::quickstart_verify_only,
    },
    Leg {
        number: 7,
        label: "adoption path: publish-only (one issuer, nothing else)",
        run: legs::quickstart_publish_only,
    },
    Leg {
        number: 8,
        label: "adoption path: augment-existing (the gateway translation edge)",
        run: legs::quickstart_gateway,
    },
    Leg {
        number: 9,
        label: "tenant isolation: cross-tenant credentials refused at the service level",
        run: legs::tenant_isolation,
    },
    Leg {
        number: 10,
        label: "NF-1: verification latency + the roll-up proof shape",
        run: legs::nf1_bench,
    },
    Leg {
        number: 11,
        label: "class claims: conform f1-f5 (FW-3)",
        run: legs::class_claims,
    },
    Leg {
        number: 12,
        label: "hub attachment: the stateless signed relay (SI-3)",
        run: legs::hub,
    },
];

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let mut harness = Harness::from_env();
    let _ = fs::create_dir_all(&harness.test_work);

    // The demo binary is the harness's own product, not a sibling's:
    // build it when missing, stop loudly when unproducible.
    if !legs::ensure_demo_binary(&harness) {
        eprintln!(
            "unidpp-harness: unidpp-demo binary unavailable at {} (cargo build failed)",
            harness.demo_bin.display()
        );
        return 1;
    }

    for leg in LEGS {
        harness.leg_header(leg.number, leg.label);
        (leg.run)(&mut harness);
    }

    harness.summary();
    i32::from(harness.fail != 0)
}
