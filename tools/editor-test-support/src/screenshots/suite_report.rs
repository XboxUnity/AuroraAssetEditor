use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use super::reference_store::{ImageStatus, ReferenceStore};
use super::screenshot_suite::SCREENS_VARIABLE;

const UPDATE_LINE: &str = "If the change is intended, run `cargo xtask ui-screenshots update` and commit the new references with the change.";
/// Characters a `file://` URL carries as they are; every other byte is percent-encoded.
const URL_SAFE_PUNCTUATION: &str = "-._~/:";
/// Wide enough for the longest path label, "reference:".
const PATH_LABEL_WIDTH: usize = 10;

/// What one run of the screenshot suite found, image by image, and what it wrote.
#[derive(Debug)]
pub struct SuiteReport {
    /// Where the references are and where this run wrote its outputs.
    pub store: ReferenceStore,
    pub areas: Vec<AreaReport>,
    /// References that have no screen in the catalogue and are still there.
    pub orphans: Vec<String>,
    /// References without a screen that this run (in update mode) deleted.
    pub removed_orphans: Vec<String>,
    /// The text screen names had to contain, when the run was limited.
    pub filter: Option<String>,
    /// Failures that belong to no single image, such as an output that could not be written.
    pub problems: Vec<String>,
    /// The contact sheet, when it could be written.
    pub sheet: Option<PathBuf>,
}

#[derive(Debug)]
pub struct AreaReport {
    pub title: &'static str,
    pub images: Vec<ImageReport>,
}

#[derive(Debug)]
pub struct ImageReport {
    pub image: String,
    pub screen: &'static str,
    pub description: &'static str,
    pub theme: &'static str,
    pub status: ImageStatus,
}

/// How many images ended in each state, for the first line of the report and the sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageCounts {
    pub screens: usize,
    pub images: usize,
    pub matching: usize,
    pub differing: usize,
    pub missing: usize,
    pub updated: usize,
    pub new: usize,
    pub errors: usize,
}

impl SuiteReport {
    pub fn images(&self) -> impl Iterator<Item = &ImageReport> {
        self.areas.iter().flat_map(|area| area.images.iter())
    }

    /// No image failed, no reference lost its screen and nothing else went wrong.
    pub fn passed(&self) -> bool {
        self.images().all(|image| !image.status.is_failure())
            && self.orphans.is_empty()
            && self.problems.is_empty()
    }

    pub fn counts(&self) -> ImageCounts {
        let screens: BTreeSet<&str> = self.images().map(|image| image.screen).collect();
        let count = |wanted: fn(&ImageStatus) -> bool| {
            self.images().filter(|image| wanted(&image.status)).count()
        };
        ImageCounts {
            screens: screens.len(),
            images: self.images().count(),
            matching: count(|status| matches!(status, ImageStatus::Matches)),
            differing: count(|status| matches!(status, ImageStatus::Differs(_))),
            missing: count(|status| matches!(status, ImageStatus::Missing)),
            updated: count(|status| matches!(status, ImageStatus::Replaced(_))),
            new: count(|status| matches!(status, ImageStatus::Created)),
            errors: count(|status| matches!(status, ImageStatus::Error(_))),
        }
    }

    /// An update run would resolve what failed: changed or missing images, or orphans.
    fn update_would_help(&self) -> bool {
        let counts = self.counts();
        counts.differing > 0 || counts.missing > 0 || !self.orphans.is_empty()
    }

    fn write_summary(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let counts = self.counts();
        let verdict = if self.passed() { "passed" } else { "FAILED" };
        writeln!(
            formatter,
            "UI screenshots {verdict}: {}, {}: {} matching, {} differing, {} missing, {} updated, {} new, {}",
            counted(counts.screens, "screen", "screens"),
            counted(counts.images, "image", "images"),
            counts.matching,
            counts.differing,
            counts.missing,
            counts.updated,
            counts.new,
            counted(counts.errors, "error", "errors")
        )?;
        if let Some(text) = &self.filter {
            writeln!(
                formatter,
                "This run was filtered to screens containing \"{text}\" ({SCREENS_VARIABLE}); orphan references were not checked."
            )?;
        }
        Ok(())
    }

    fn write_failures(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let failures: Vec<&ImageReport> = self
            .images()
            .filter(|image| image.status.is_failure())
            .collect();
        if failures.is_empty() {
            return Ok(());
        }
        writeln!(formatter, "\nFailed images:")?;
        for image in failures {
            let name = image.image.as_str();
            match &image.status {
                ImageStatus::Differs(comparison) => {
                    writeln!(formatter, "  {name}: {comparison}")?;
                    write_path(formatter, "reference", &self.store.reference_path(name))?;
                    write_path(formatter, "actual", &self.store.actual_path(name))?;
                    write_path(formatter, "diff", &self.store.diff_path(name))?;
                }
                ImageStatus::Missing => {
                    let reference = self.store.reference_path(name);
                    writeln!(formatter, "  {name}: no reference {}", reference.display())?;
                    write_path(formatter, "actual", &self.store.actual_path(name))?;
                }
                ImageStatus::Error(message) => writeln!(formatter, "  {name}: {message}")?,
                ImageStatus::Matches | ImageStatus::Created | ImageStatus::Replaced(_) => {}
            }
        }
        Ok(())
    }

    fn write_reference_changes(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let changed: Vec<&ImageReport> = self
            .images()
            .filter(|image| {
                matches!(
                    image.status,
                    ImageStatus::Replaced(_) | ImageStatus::Created
                )
            })
            .collect();
        if !changed.is_empty() {
            writeln!(formatter, "\nReferences written:")?;
        }
        for image in changed {
            let name = image.image.as_str();
            match &image.status {
                ImageStatus::Replaced(comparison) => {
                    writeln!(formatter, "  {name} updated: {comparison}")?;
                    write_path(formatter, "old", &self.store.reference_copy_path(name))?;
                    write_path(formatter, "new", &self.store.reference_path(name))?;
                    write_path(formatter, "diff", &self.store.diff_path(name))?;
                }
                ImageStatus::Created => {
                    writeln!(formatter, "  {name} new")?;
                    write_path(formatter, "new", &self.store.reference_path(name))?;
                }
                ImageStatus::Matches
                | ImageStatus::Differs(_)
                | ImageStatus::Missing
                | ImageStatus::Error(_) => {}
            }
        }
        if !self.removed_orphans.is_empty() {
            writeln!(
                formatter,
                "\nRemoved references without a screen: {}",
                self.removed_orphans.join(", ")
            )?;
        }
        Ok(())
    }

    fn write_orphans(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.orphans.is_empty() {
            return Ok(());
        }
        writeln!(
            formatter,
            "\nReferences without a screen in the catalogue (renamed or removed screens):"
        )?;
        for orphan in &self.orphans {
            write_path(formatter, orphan, &self.store.reference_path(orphan))?;
        }
        Ok(())
    }

    fn write_problems(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.problems.is_empty() {
            return Ok(());
        }
        writeln!(formatter, "\nProblems:")?;
        for problem in &self.problems {
            writeln!(formatter, "  {problem}")?;
        }
        Ok(())
    }

    fn write_sheet(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter)?;
        if self.update_would_help() {
            writeln!(formatter, "{UPDATE_LINE}")?;
        }
        match &self.sheet {
            Some(sheet) => writeln!(formatter, "Contact sheet: {}", file_url(sheet)),
            None => writeln!(
                formatter,
                "Contact sheet: not written (see the problems above)"
            ),
        }
    }
}

impl fmt::Display for SuiteReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_summary(formatter)?;
        self.write_failures(formatter)?;
        self.write_reference_changes(formatter)?;
        self.write_orphans(formatter)?;
        self.write_problems(formatter)?;
        self.write_sheet(formatter)
    }
}

fn counted(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}

/// The labels of one image's paths line up, so the paths start in one column.
fn write_path(formatter: &mut fmt::Formatter<'_>, label: &str, path: &Path) -> fmt::Result {
    let label = format!("{label}:");
    writeln!(
        formatter,
        "    {label:<PATH_LABEL_WIDTH$} {}",
        path.display()
    )
}

/// `D:\target\index.html` becomes `file:///D:/target/index.html`, which a terminal can open.
fn file_url(path: &Path) -> String {
    let slashed = path.display().to_string().replace('\\', "/");
    let mut url = String::from("file://");
    if !slashed.starts_with('/') {
        url.push('/');
    }
    for byte in slashed.bytes() {
        let character = char::from(byte);
        if character.is_ascii_alphanumeric() || URL_SAFE_PUNCTUATION.contains(character) {
            url.push(character);
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}
