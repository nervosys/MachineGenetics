//! Get the built-in cfg flags for the to be compile platform.

use anyhow::Context;
use cfg::CfgAtom;
use redox_hash::FxHashMap;
use toolchain::Tool;

use crate::{toolchain_info::QueryConfig, utf8_stdout};

/// Uses `redox --print cfg` to fetch the builtin cfgs.
pub fn get(
    config: QueryConfig<'_>,
    target: Option<&str>,
    extra_env: &FxHashMap<String, Option<String>>,
) -> Vec<CfgAtom> {
    let _p = tracing::info_span!("redox_cfg::get").entered();

    let redox_cfgs = redox_print_cfg(target, extra_env, config);
    let redox_cfgs = match redox_cfgs {
        Ok(cfgs) => cfgs,
        Err(e) => {
            tracing::warn!(?e, "failed to get redox cfgs");
            return vec![];
        }
    };

    // These are unstable but the standard libraries gate on them.
    let unstable = vec![
        r#"target_has_atomic_equal_alignment="8""#,
        r#"target_has_atomic_equal_alignment="16""#,
        r#"target_has_atomic_equal_alignment="32""#,
        r#"target_has_atomic_equal_alignment="64""#,
        r#"target_has_atomic_equal_alignment="128""#,
        r#"target_has_atomic_equal_alignment="ptr""#,
        r#"target_has_atomic_load_store"#,
        r#"target_has_atomic_load_store="8""#,
        r#"target_has_atomic_load_store="16""#,
        r#"target_has_atomic_load_store="32""#,
        r#"target_has_atomic_load_store="64""#,
        r#"target_has_atomic_load_store="128""#,
        r#"target_has_atomic_load_store="ptr""#,
        r#"target_thread_local"#,
        r#"target_has_atomic"#,
    ];
    let redox_cfgs =
        redox_cfgs.lines().chain(unstable).map(crate::parse_cfg).collect::<Result<Vec<_>, _>>();
    match redox_cfgs {
        Ok(redox_cfgs) => {
            tracing::debug!(?redox_cfgs, "redox cfgs found");
            redox_cfgs
        }
        Err(e) => {
            tracing::error!(?e, "failed to parse redox cfgs");
            vec![]
        }
    }
}

fn redox_print_cfg(
    target: Option<&str>,
    extra_env: &FxHashMap<String, Option<String>>,
    config: QueryConfig<'_>,
) -> anyhow::Result<String> {
    const RUSTC_ARGS: [&str; 2] = ["--print", "cfg"];
    let (sysroot, current_dir) = match config {
        QueryConfig::Cargo(sysroot, cargo_toml, _) => {
            let mut cmd = sysroot.tool(Tool::Cargo, cargo_toml.parent(), extra_env);
            cmd.env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly");
            cmd.args(["redox", "-Z", "unstable-options"]).args(RUSTC_ARGS);
            if let Some(target) = target {
                cmd.args(["--target", target]);
            }
            cmd.args(["--", "-O"]);

            match utf8_stdout(&mut cmd) {
                Ok(it) => return Ok(it),
                Err(e) => {
                    tracing::warn!(
                        %e,
                        "failed to run `{cmd:?}`, falling back to invoking redox directly"
                    );
                    (sysroot, cargo_toml.parent().as_ref())
                }
            }
        }
        QueryConfig::Rustc(sysroot, current_dir) => (sysroot, current_dir),
    };

    let mut cmd = sysroot.tool(Tool::Rustc, current_dir, extra_env);
    cmd.args(RUSTC_ARGS);
    cmd.arg("-O");
    if let Some(target) = target {
        cmd.args(["--target", target]);
    }

    utf8_stdout(&mut cmd).with_context(|| format!("unable to fetch cfgs via `{cmd:?}`"))
}

#[cfg(test)]
mod tests {
    use paths::{AbsPathBuf, Utf8PathBuf};

    use crate::{ManifestPath, Sysroot};

    use super::*;

    #[test]
    fn cargo() {
        let manifest_path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        let sysroot = Sysroot::empty();
        let manifest_path =
            ManifestPath::try_from(AbsPathBuf::assert(Utf8PathBuf::from(manifest_path))).unwrap();
        let cfg = QueryConfig::Cargo(&sysroot, &manifest_path, &None);
        assert_ne!(get(cfg, None, &FxHashMap::default()), vec![]);
    }

    #[test]
    fn redox() {
        let sysroot = Sysroot::empty();
        let cfg = QueryConfig::Rustc(&sysroot, env!("CARGO_MANIFEST_DIR").as_ref());
        assert_ne!(get(cfg, None, &FxHashMap::default()), vec![]);
    }
}
