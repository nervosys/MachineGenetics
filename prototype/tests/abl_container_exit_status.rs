//! A corrupt Agentic Binary Language container must not exit 0.
//!
//! Both container entry points — `--from=abl-bytes` (decompile) and
//! `--run=abl-bytes` (dispatch) — hand-roll the same `take()`-guarded decoder.
//! The guards work: eight malformed containers, including length fields at the
//! `u32` ceiling, produce a diagnostic and no panic. But every one of them
//! *also* produced **exit 0**, while the same match arm exits 1 when the file
//! merely cannot be read. A caller driving the tool by exit status — which is
//! how an agent drives it — could not tell a decoded container from a rejected
//! one.
//!
//! That is the taxonomy's "detected, recorded, and never surfaced", which the
//! handoff notes is indistinguishable from not detected.
//!
//! These are process-level tests because the failure is a process exit status;
//! `std::process::exit` cannot be observed from inside the binary's own test
//! harness. The happy-path cases are the half that matters most: they are what
//! fails if someone later makes the decoder exit non-zero unconditionally,
//! which would "fix" the corrupt cases while breaking every real one.

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_mage-parse");

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `prototype/`; the sources live beside it.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("prototype/ has a parent")
        .to_path_buf()
}

/// A directory unique to this test binary, so a parallel run cannot collide.
/// No timestamp or RNG: the process id is enough and keeps the test
/// deterministic to read.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mage-abl-exit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir.join(name)
}

fn status_of(flag: &str, path: &Path) -> i32 {
    let out = Command::new(BIN)
        .arg(flag)
        .arg(path)
        .output()
        .unwrap_or_else(|e| panic!("spawn {BIN}: {e}"));
    out.status.code().unwrap_or_else(|| {
        panic!(
            "{flag} on {} was killed by a signal rather than exiting",
            path.display()
        )
    })
}

/// Lower a real `.mg` source to a container, so the happy-path assertions run
/// against bytes the tool itself produced rather than a committed fixture that
/// could drift from the format.
fn good_container() -> PathBuf {
    let src = repo_root().join("benchmarks/constructs/mlp_mage.mg");
    assert!(src.is_file(), "missing source: {}", src.display());

    let out = scratch("good.abl");
    let st = Command::new(BIN)
        .arg("--target=abl-bytes")
        .arg(&src)
        .arg(&out)
        .output()
        .expect("spawn mage-parse --target=abl-bytes");

    assert!(
        st.status.success(),
        "lowering {} failed: {}",
        src.display(),
        String::from_utf8_lossy(&st.stderr)
    );
    assert!(out.is_file(), "no container written to {}", out.display());
    out
}

fn write(name: &str, bytes: &[u8]) -> PathBuf {
    let p = scratch(name);
    std::fs::write(&p, bytes).expect("write container");
    p
}

const MAGIC: &[u8; 4] = b"ABL1";

/// Header for `count` items, with `body` appended verbatim.
fn container(version: u16, count: u32, body: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(MAGIC);
    v.extend_from_slice(&version.to_le_bytes());
    v.extend_from_slice(&count.to_le_bytes());
    v.extend_from_slice(body);
    v
}

/// Read from the crate rather than hardcoded, so a container-version bump
/// cannot leave these cases silently testing the "unsupported version" path
/// instead of the one each case is named for.
fn version() -> u16 {
    mage_prototype::abl::ABL_VERSION
}

/// Every corrupt container, through both entry points, must exit non-zero.
///
/// The cases are the ones that distinguish a real bounds check from an absent
/// one: two claim a length at the `u32` ceiling, which is what would overflow
/// the `*pos + n` bound on a 32-bit target, and one claims four billion items.
#[test]
fn a_corrupt_container_exits_non_zero_through_both_entry_points() {
    let v = version();
    let mut body_huge_name = Vec::new();
    body_huge_name.extend_from_slice(&u32::MAX.to_le_bytes());
    body_huge_name.extend_from_slice(b"ab");

    let mut body_huge_expr = Vec::new();
    body_huge_expr.extend_from_slice(&2u32.to_le_bytes());
    body_huge_expr.extend_from_slice(b"ab");
    body_huge_expr.extend_from_slice(&u32::MAX.to_le_bytes());
    body_huge_expr.push(b'x');

    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("truncated_header", MAGIC.iter().copied().chain([0x03]).collect()),
        ("no_item_bytes", container(v, 1, &[])),
        ("huge_count", container(v, u32::MAX, &[])),
        ("huge_name_len", container(v, 1, &body_huge_name)),
        ("huge_expr_len", container(v, 1, &body_huge_expr)),
        ("bad_version", container(u16::MAX, 1, &[])),
        ("bad_magic", b"NOPE\x03\x00\x01\x00\x00\x00".to_vec()),
    ];

    for (name, bytes) in cases {
        let path = write(&format!("bad_{name}.abl"), &bytes);
        for flag in ["--from=abl-bytes", "--run=abl-bytes"] {
            assert_ne!(
                status_of(flag, &path),
                0,
                "{flag} reported success on the `{name}` container. \
                 A caller reading the exit status cannot tell it was rejected."
            );
        }
    }
}

/// The other direction, and the one that catches an over-eager fix: a valid
/// container must still exit 0 through both entry points.
#[test]
fn a_valid_container_still_exits_zero_through_both_entry_points() {
    let good = good_container();
    for flag in ["--from=abl-bytes", "--run=abl-bytes"] {
        assert_eq!(
            status_of(flag, &good),
            0,
            "{flag} failed on a container this tool had just produced"
        );
    }
}
