//! The version players see: in the game's corner, on the browser's start
//! screen, and on every Short made from the app, so a viewer can tell which
//! version a video shows. `Cargo.toml` holds the number and `CHANGELOG.md`
//! what each version changed.

/// The version as the game shows it: `v0.7` for 0.7.0 and `v0.7.1` for
/// 0.7.1. `tools/build_web.ps1` writes it the same way on the start screen.
pub fn version_label() -> String {
    let version = env!("CARGO_PKG_VERSION");
    match version.rsplit_once('.') {
        Some((major_minor, "0")) => format!("v{major_minor}"),
        _ => format!("v{version}"),
    }
}

#[cfg(test)]
mod tests {
    use super::version_label;

    /// The start screen shows the newest entry of `CHANGELOG.md` as what is
    /// new in this version, so a bump needs its entry, in the form
    /// `tools/build_web.ps1` reads: `## v0.7 (2026-10-12)` and a `- ` line for
    /// each change.
    #[test]
    fn changelog_starts_with_this_version() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("CHANGELOG.md");
        let changelog = std::fs::read_to_string(path).unwrap();
        let mut entries = changelog.split("\n## ").skip(1);
        let newest = entries.next().unwrap_or_default();
        let (heading, changes) = newest.split_once('\n').unwrap_or((newest, ""));
        let label = version_label();
        let date = heading
            .trim_end()
            .strip_prefix(&format!("{label} ("))
            .and_then(|rest| rest.strip_suffix(')'));
        assert!(
            date.is_some_and(|date| {
                date.len() == 10
                    && date
                        .char_indices()
                        .all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() })
            }),
            "CHANGELOG.md should start with an entry for {label}, headed `## {label} (YYYY-MM-DD)`, \
             not `## {heading}`"
        );
        assert!(
            changes.lines().any(|line| line.starts_with("- ")),
            "CHANGELOG.md's entry for {label} should list its changes as `- ` lines"
        );
    }
}
