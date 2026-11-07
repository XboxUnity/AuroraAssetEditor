const APP_NAME: &str = "Aurora Asset Editor";
const CREDITS: &str = "by Swizzy thanks to MaesterRowen";
const DIRTY_MARK: &str = ".dirty";

/// What was built, from which commit, when and for which target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    pub version: String,
    pub short_hash: String,
    pub full_hash: String,
    /// The working tree had uncommitted changes.
    pub dirty: bool,
    pub build_date: String,
    pub target: String,
    /// The CI run that built it; `None` for local builds.
    pub run_url: Option<String>,
}

impl BuildInfo {
    /// The build this program is: the values `build.rs` captured when it was compiled.
    pub fn current() -> BuildInfo {
        let run_url = env!("AURORA_BUILD_RUN_URL");
        BuildInfo {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            short_hash: env!("AURORA_GIT_HASH").to_owned(),
            full_hash: env!("AURORA_GIT_HASH_FULL").to_owned(),
            dirty: env!("AURORA_GIT_DIRTY") == "true",
            build_date: env!("AURORA_BUILD_DATE").to_owned(),
            target: env!("AURORA_TARGET").to_owned(),
            run_url: (!run_url.is_empty()).then(|| run_url.to_owned()),
        }
    }

    /// "2.0.7 (a1b2c3d)", or "2.0.7 (a1b2c3d.dirty)" for a dirty tree.
    pub fn pill_text(&self) -> String {
        let dirty_mark = if self.dirty { DIRTY_MARK } else { "" };
        format!("{} ({}{dirty_mark})", self.version, self.short_hash)
    }

    /// The old title, `Aurora Asset Editor v{0}.{1}.{2} by Swizzy thanks to MaesterRowen`.
    pub fn window_title(&self) -> String {
        format!("{APP_NAME} v{} {CREDITS}", self.version)
    }

    /// The About dialog's "Copy build info" text: four lines, five with a CI run.
    pub fn copy_text(&self) -> String {
        let mut lines = vec![
            format!("{APP_NAME} {}", self.pill_text()),
            format!("Commit: {}", self.full_hash),
            format!("Built: {}", self.build_date),
            format!("Target: {}", self.target),
        ];
        if let Some(run_url) = &self.run_url {
            lines.push(format!("Run: {run_url}"));
        }
        lines.join("\n")
    }
}
