//! Discovery of an F´ project from its `settings.ini`.
//!
//! `settings.ini` marks the root of an F´ project. `fprime-util generate` creates
//! the build caches next to it, named `build-fprime-automatic-<toolchain>`, and
//! each build cache holds the `locs.fpp` that locates every definition in the
//! project.

use std::path::{Path, PathBuf};

/// F´ settings file that records the project layout.
pub const SETTINGS_FILE: &str = "settings.ini";

/// The locs file written into each build cache.
pub const LOCS_FILE: &str = "locs.fpp";

/// The toolchain `fprime-util` builds with when `settings.ini` names none.
const DEFAULT_TOOLCHAIN: &str = "native";

/// The `[fprime]` settings of an F´ project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The directory containing `settings.ini`.
    pub dir: PathBuf,
    /// `default_toolchain`, as written.
    pub default_toolchain: Option<String>,
}

impl Settings {
    /// Read the `settings.ini` in `dir`, if there is one.
    ///
    /// Only the keys used here are parsed; this intentionally avoids taking on an
    /// INI-parser dependency for the format.
    pub fn read(dir: &Path) -> Option<Settings> {
        let contents = std::fs::read_to_string(dir.join(SETTINGS_FILE)).ok()?;
        let mut settings = Settings {
            dir: dir.to_path_buf(),
            default_toolchain: None,
        };

        let mut in_fprime_section = false;
        for raw in contents.lines() {
            // An indented line continues the value of the key before it.
            if raw.starts_with([' ', '\t']) {
                continue;
            }
            let line = raw.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some(section) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                in_fprime_section = section.trim().eq_ignore_ascii_case("fprime");
                continue;
            }
            if !in_fprime_section {
                continue;
            }
            // Keys may use `:` or `=` as the separator.
            let Some(idx) = line.find([':', '=']) else {
                continue;
            };
            let key = line[..idx].trim();
            let value = line[idx + 1..].trim();
            if value.is_empty() {
                continue;
            }
            if key.eq_ignore_ascii_case("default_toolchain") {
                settings
                    .default_toolchain
                    .get_or_insert_with(|| value.to_string());
            }
        }

        Some(settings)
    }

    /// Read the `settings.ini` of the project containing `path`: the nearest one
    /// in `path` or one of its ancestors.
    pub fn find(path: &Path) -> Option<Settings> {
        path.ancestors().find_map(Settings::read)
    }

    /// The build cache `fprime-util generate` creates for the default toolchain.
    pub fn build_cache(&self) -> PathBuf {
        let toolchain = self
            .default_toolchain
            .as_deref()
            .unwrap_or(DEFAULT_TOOLCHAIN);
        self.dir.join(format!("build-fprime-automatic-{toolchain}"))
    }

    /// The locs file in the default build cache. It exists once the project has
    /// been generated.
    pub fn locs_file(&self) -> PathBuf {
        self.build_cache().join(LOCS_FILE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A unique temp dir per test without pulling in a tempfile dependency.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fpp-fs-fprime-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_default_toolchain() {
        let dir = temp_dir("read");
        std::fs::write(
            dir.join(SETTINGS_FILE),
            "; a comment\n[fprime]\nframework_path: ..\nproject_root: .\n\
             default_toolchain: gcc-arm\n",
        )
        .unwrap();
        let settings = Settings::read(&dir).unwrap();
        assert_eq!(settings.default_toolchain.as_deref(), Some("gcc-arm"));
        assert_eq!(
            settings.locs_file(),
            dir.join("build-fprime-automatic-gcc-arm").join(LOCS_FILE)
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_equals_separator() {
        let dir = temp_dir("eq");
        std::fs::write(
            dir.join(SETTINGS_FILE),
            "[fprime]\ndefault_toolchain = arm\n",
        )
        .unwrap();
        let settings = Settings::read(&dir).unwrap();
        assert_eq!(settings.default_toolchain.as_deref(), Some("arm"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ignores_keys_outside_fprime_section() {
        let dir = temp_dir("section");
        std::fs::write(
            dir.join(SETTINGS_FILE),
            "[other]\ndefault_toolchain: nope\n",
        )
        .unwrap();
        let settings = Settings::read(&dir).unwrap();
        assert_eq!(
            settings.build_cache(),
            dir.join("build-fprime-automatic-native")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ignores_continuation_lines() {
        let dir = temp_dir("continuation");
        std::fs::write(
            dir.join(SETTINGS_FILE),
            "[fprime]\ndefault_cmake_options: A=1\n    default_toolchain=nope\n",
        )
        .unwrap();
        assert_eq!(Settings::read(&dir).unwrap().default_toolchain, None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_settings_is_none() {
        let dir = temp_dir("missing");
        assert_eq!(Settings::read(&dir), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn finds_settings_in_an_ancestor() {
        let dir = temp_dir("find");
        std::fs::write(dir.join(SETTINGS_FILE), "[fprime]\n").unwrap();
        let nested = dir.join("Components").join("Foo");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(Settings::find(&nested.join("Foo.fpp")).unwrap().dir, dir);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
