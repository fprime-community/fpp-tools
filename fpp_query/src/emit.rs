//! Turning matched stems into the output list.
//!
//! The format is CMake's, not ours. `file(STRINGS)` reads the result, so:
//! absolute paths (they go straight into `add_custom_command(OUTPUT ...)`), one
//! per line, LF-terminated, no blank lines (a blank line becomes an empty list
//! element, hence an empty `OUTPUT`), and the file must exist even when nothing
//! matched, because `file(STRINGS)` on a missing file is a hard `FATAL_ERROR`.

use crate::select::{Group, Matches};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub fn render(groups: &[Group], matches: &Matches, directory: &str) -> String {
    let base: PathBuf = fpp_core::File::get_path(directory);

    let mut paths: BTreeSet<String> = BTreeSet::new();
    for (group, stems) in groups.iter().zip(&matches.stems) {
        for stem in stems {
            for suffix in &group.suffixes {
                paths.insert(join(&base, stem, suffix));
            }
        }
    }

    let mut out = String::with_capacity(paths.len() * 96);
    for path in paths {
        out.push_str(&path);
        out.push('\n');
    }
    out
}

fn join(base: &Path, stem: &str, suffix: &str) -> String {
    base.join(format!("{stem}{suffix}"))
        .to_string_lossy()
        .into_owned()
}

/// Write `text` to `path`, creating parent directories.
pub fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}
