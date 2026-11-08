use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use super::image_change::ImageChange;
use super::pixel_comparison::PixelComparison;
use super::run_folder::RunFolder;
use super::run_manifest::RunManifest;
use super::screenshot_suite::SCREENS_VARIABLE;

const REVIEW_LINE: &str = "Changes do not fail the run; review them on the contact sheet.";
/// Characters a `file://` URL carries as they are; every other byte is percent-encoded.
const URL_SAFE_PUNCTUATION: &str = "-._~/:";
/// Wide enough for the longest path label, "baseline:".
const PATH_LABEL_WIDTH: usize = 9;

/// What one run of the screenshot suite found, image by image, and what it wrote.
#[derive(Debug)]
pub struct SuiteReport {
    /// Where this run wrote its images.
    pub folder: RunFolder,
    pub platform: String,
    /// What this run is called, such as "local run" or a release.
    pub label: String,
    /// When this run started, as `%Y-%m-%d %H:%M UTC`.
    pub rendered: String,
    /// The manifest of the run this one was compared with, when there was one.
    pub baseline: Option<RunManifest>,
    pub areas: Vec<AreaReport>,
    /// Baseline images without a current image: their screens were removed or renamed.
    pub removed: Vec<String>,
    /// The text screen names had to contain, when the run was limited.
    pub filter: Option<String>,
    /// What the reviewer should know that is no failure, such as an earlier run being deleted.
    pub notes: Vec<String>,
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
    pub change: ImageChange,
}

/// How many images ended in each state, for the first line of the report and the sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageCounts {
    pub screens: usize,
    pub images: usize,
    pub changed: usize,
    pub new: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub unreadable_baselines: usize,
    pub errors: usize,
}

impl SuiteReport {
    pub fn images(&self) -> impl Iterator<Item = &ImageReport> {
        self.areas.iter().flat_map(|area| area.images.iter())
    }

    /// No image failed and nothing else went wrong. Changed, new and removed images are for the
    /// review and never fail a run.
    pub fn passed(&self) -> bool {
        self.images().all(|image| !image.change.is_failure()) && self.problems.is_empty()
    }

    pub fn counts(&self) -> ImageCounts {
        let screens: BTreeSet<&str> = self.images().map(|image| image.screen).collect();
        let count = |wanted: fn(&ImageChange) -> bool| {
            self.images().filter(|image| wanted(&image.change)).count()
        };
        ImageCounts {
            screens: screens.len(),
            images: self.images().count(),
            changed: count(|change| matches!(change, ImageChange::Changed(_))),
            new: count(|change| matches!(change, ImageChange::New)),
            removed: self.removed.len(),
            unchanged: count(|change| matches!(change, ImageChange::Unchanged)),
            unreadable_baselines: count(|change| {
                matches!(change, ImageChange::BaselineUnreadable(_))
            }),
            errors: count(|change| matches!(change, ImageChange::Error(_))),
        }
    }

    fn write_summary(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let counts = self.counts();
        let verdict = if self.passed() { "passed" } else { "FAILED" };
        writeln!(
            formatter,
            "UI screenshots {verdict}: {}, {}: {} changed, {} new, {} removed, {} unchanged, {}, {}",
            counted(counts.screens, "screen", "screens"),
            counted(counts.images, "image", "images"),
            counts.changed,
            counts.new,
            counts.removed,
            counts.unchanged,
            counted(
                counts.unreadable_baselines,
                "unreadable baseline",
                "unreadable baselines"
            ),
            counted(counts.errors, "error", "errors")
        )?;
        match &self.baseline {
            Some(baseline) => writeln!(
                formatter,
                "Compared with: {}, rendered {} ({})",
                baseline.label, baseline.rendered, baseline.platform
            )?,
            None => writeln!(formatter, "Compared with: nothing, so every image is new")?,
        }
        if let Some(text) = &self.filter {
            writeln!(
                formatter,
                "This run was filtered to screens containing \"{text}\" ({SCREENS_VARIABLE}); removed screens were not looked for."
            )?;
        }
        if counts.changed > 0 || counts.new > 0 || counts.removed > 0 {
            writeln!(formatter, "{REVIEW_LINE}")?;
        }
        Ok(())
    }

    fn write_changed(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let changed: Vec<(&str, &PixelComparison)> = self
            .images()
            .filter_map(|image| match &image.change {
                ImageChange::Changed(comparison) => Some((image.image.as_str(), comparison)),
                _ => None,
            })
            .collect();
        if changed.is_empty() {
            return Ok(());
        }
        writeln!(formatter, "\nChanged:")?;
        for (name, comparison) in changed {
            writeln!(formatter, "  {name}: {comparison}")?;
            write_path(formatter, "baseline", &self.folder.baseline_copy_path(name))?;
            write_path(formatter, "current", &self.folder.image_path(name))?;
            write_path(formatter, "diff", &self.folder.diff_path(name))?;
        }
        Ok(())
    }

    fn write_new(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let new: Vec<String> = self
            .images()
            .filter(|image| image.change == ImageChange::New)
            .map(|image| image.image.clone())
            .collect();
        write_list(formatter, "New", &new)
    }

    fn write_removed(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.removed.is_empty() {
            return Ok(());
        }
        writeln!(formatter, "\nRemoved:")?;
        for name in &self.removed {
            writeln!(formatter, "  {name}")?;
            write_path(formatter, "baseline", &self.folder.baseline_copy_path(name))?;
        }
        Ok(())
    }

    /// The reason of an unreadable baseline image or the message of an error, after its name.
    fn write_messages(
        &self,
        formatter: &mut fmt::Formatter<'_>,
        heading: &str,
        message_of: fn(&ImageChange) -> Option<&str>,
    ) -> fmt::Result {
        let lines: Vec<String> = self
            .images()
            .filter_map(|image| {
                message_of(&image.change).map(|message| format!("{}: {message}", image.image))
            })
            .collect();
        write_list(formatter, heading, &lines)
    }

    fn write_sheet(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter)?;
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
        self.write_changed(formatter)?;
        self.write_new(formatter)?;
        self.write_removed(formatter)?;
        self.write_messages(
            formatter,
            "Unreadable baseline images",
            |change| match change {
                ImageChange::BaselineUnreadable(reason) => Some(reason),
                _ => None,
            },
        )?;
        self.write_messages(formatter, "Errors", |change| match change {
            ImageChange::Error(message) => Some(message),
            _ => None,
        })?;
        write_list(formatter, "Notes", &self.notes)?;
        write_list(formatter, "Problems", &self.problems)?;
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

/// A heading and one indented line per item; nothing when there are no items.
fn write_list(formatter: &mut fmt::Formatter<'_>, heading: &str, items: &[String]) -> fmt::Result {
    if items.is_empty() {
        return Ok(());
    }
    writeln!(formatter, "\n{heading}:")?;
    for item in items {
        writeln!(formatter, "  {item}")?;
    }
    Ok(())
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
