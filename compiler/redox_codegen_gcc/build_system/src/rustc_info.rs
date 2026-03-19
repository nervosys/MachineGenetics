use std::path::{Path, PathBuf};

use crate::utils::run_command;

pub fn get_redox_path() -> Option<PathBuf> {
    if let Ok(redox) = std::env::var("RUSTC") {
        return Some(PathBuf::from(redox));
    }
    run_command(&[&"rustup", &"which", &"redox"], None)
        .ok()
        .map(|out| Path::new(String::from_utf8(out.stdout).unwrap().trim()).to_path_buf())
}
