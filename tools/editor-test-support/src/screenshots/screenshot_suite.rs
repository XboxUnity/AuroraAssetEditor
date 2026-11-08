use std::any::Any;
use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;

use aurora_asset_editor::settings::ThemeChoice;
use image::RgbaImage;

use super::baseline::Baseline;
use super::contact_sheet::{CONTACT_SHEET_FILE, write_contact_sheet};
use super::image_change::ImageChange;
use super::path_error::with_path;
use super::run_manifest::RunManifest;
use super::scene::Scene;
use super::screen::{SCREEN_THEMES, Screen, ScreenArea, image_name};
use super::screenshot_folders::ScreenshotFolders;
use super::suite_report::{AreaReport, ImageReport, SuiteReport};

/// Limits a run to the screens whose name contains its value.
pub const SCREENS_VARIABLE: &str = "AURORA_UI_SCREENS";
/// The folder to compare with instead of the previous local run.
pub const BASELINE_VARIABLE: &str = "AURORA_UI_BASELINE";
/// What the run is called on the contact sheet and in its manifest.
pub const LABEL_VARIABLE: &str = "AURORA_UI_LABEL";
pub const DEFAULT_LABEL: &str = "local run";
/// The run report as plain text, next to the contact sheet: the same text a failing test prints.
pub const REPORT_FILE: &str = "report.txt";

const CI_VARIABLE: &str = "CI";
const RENDERED_FORMAT: &str = "%Y-%m-%d %H:%M UTC";
const PANIC_WITHOUT_TEXT: &str = "a panic without a message";

/// What a run compares its images with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaselineSource {
    /// The last finished unfiltered local run of this platform, if there is one.
    PreviousRun,
    /// A finished run of this platform, such as one from the previous release's zip.
    Folder(PathBuf),
}

/// Captures every screen of a catalogue in both themes and compares each image with a baseline
/// from the same platform.
pub struct ScreenshotSuite {
    folders: ScreenshotFolders,
    baseline: BaselineSource,
    only: Option<String>,
    label: String,
}

impl ScreenshotSuite {
    /// `only` limits the run to the screens whose name contains it; such a run cannot tell which
    /// screens were removed, so it does not look for them.
    pub fn new(
        folders: ScreenshotFolders,
        baseline: BaselineSource,
        only: Option<String>,
        label: String,
    ) -> ScreenshotSuite {
        ScreenshotSuite {
            folders,
            baseline,
            only,
            label,
        }
    }

    /// The workspace's output folders, the baseline from `AURORA_UI_BASELINE`, the label from
    /// `AURORA_UI_LABEL` and the filter from `AURORA_UI_SCREENS`. Panics on a filter the run must
    /// refuse, naming why.
    pub fn from_environment() -> ScreenshotSuite {
        let only = ScreenshotSuite::filter_from_values(
            read_variable(SCREENS_VARIABLE).as_deref(),
            read_variable(CI_VARIABLE).as_deref(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        ScreenshotSuite::new(
            ScreenshotFolders::for_workspace(),
            ScreenshotSuite::baseline_from_value(read_variable(BASELINE_VARIABLE).as_deref()),
            only,
            ScreenshotSuite::label_from_value(read_variable(LABEL_VARIABLE).as_deref()),
        )
    }

    /// An unset or empty `screens` means every screen. A filter is refused when `ci` is set to
    /// anything non-empty: a filtered CI run would give the next release a partial baseline.
    pub fn filter_from_values(
        screens: Option<&str>,
        ci: Option<&str>,
    ) -> Result<Option<String>, ScreenFilterOnCi> {
        let filter = screens.filter(|text| !text.is_empty());
        if filter.is_some() && ci.is_some_and(|value| !value.is_empty()) {
            return Err(ScreenFilterOnCi);
        }
        Ok(filter.map(str::to_owned))
    }

    /// An unset or empty value means the previous local run.
    pub fn baseline_from_value(value: Option<&str>) -> BaselineSource {
        match value.filter(|text| !text.is_empty()) {
            Some(folder) => BaselineSource::Folder(PathBuf::from(folder)),
            None => BaselineSource::PreviousRun,
        }
    }

    pub fn label_from_value(value: Option<&str>) -> String {
        value
            .filter(|text| !text.is_empty())
            .unwrap_or(DEFAULT_LABEL)
            .to_owned()
    }

    /// The earlier report and sheet go first and the output folder is prepared next; a failure in either
    /// stops the run before anything is rendered. Whatever happens after that, the run ends by
    /// writing the sheet, the text report and the manifest, so a failed run is still a baseline
    /// for the next one.
    pub fn run(&self, areas: &[ScreenArea]) -> SuiteReport {
        let mut report = SuiteReport {
            folder: self.folders.current(),
            platform: self.folders.platform().to_owned(),
            label: self.label.clone(),
            rendered: chrono::Utc::now().format(RENDERED_FORMAT).to_string(),
            baseline: None,
            areas: Vec::new(),
            removed: Vec::new(),
            filter: self.only.clone(),
            notes: Vec::new(),
            problems: Vec::new(),
            sheet: None,
        };
        if let Err(message) = remove_earlier_results(report.folder.folder()) {
            report
                .problems
                .push(format!("{message}, so nothing was rendered"));
            return report;
        }
        let baseline_allowed = self.check_baseline_folder(&mut report);
        match self.folders.start_run() {
            Ok(note) => report.notes.extend(note),
            Err(error) => {
                report.problems.push(format!(
                    "Could not prepare the output folder, so nothing was rendered: {error}"
                ));
                return report;
            }
        }
        let baseline = if baseline_allowed {
            self.open_baseline(&mut report)
        } else {
            None
        };
        report.baseline = baseline
            .as_ref()
            .map(|baseline| baseline.manifest().clone());

        for area in areas {
            let images: Vec<ImageReport> = area
                .screens
                .iter()
                .filter(|screen| self.includes(screen))
                .flat_map(|screen| self.record_screen(screen, &report, baseline.as_ref()))
                .collect();
            if !images.is_empty() {
                report.areas.push(AreaReport {
                    title: area.title,
                    images,
                });
            }
        }
        self.check_catalogue(&mut report);
        if self.only.is_none()
            && let Some(baseline) = &baseline
        {
            keep_removed_images(baseline, &mut report);
        }
        self.write_results(&mut report);
        report
    }

    /// The rotation at the start of the run replaces the current run folder, so a baseline there
    /// would be gone before it is read. Checked before anything is touched.
    fn check_baseline_folder(&self, report: &mut SuiteReport) -> bool {
        let BaselineSource::Folder(folder) = &self.baseline else {
            return true;
        };
        if folder.is_relative() {
            report.problems.push(format!(
                "{BASELINE_VARIABLE} is the relative path {}; `cargo test` runs in the package folder, so the baseline path must be absolute",
                folder.display()
            ));
            return false;
        }
        let current = report.folder.folder();
        match is_same_or_inside(folder, current) {
            Ok(false) => true,
            Ok(true) => {
                report.problems.push(format!(
                    "The baseline {} is the run folder {} or inside it; each run replaces that folder, so it cannot hold the baseline",
                    folder.display(),
                    current.display()
                ));
                false
            }
            Err(error) => {
                report.problems.push(format!(
                    "Could not tell whether the baseline {} is inside the run folder {}: {error}",
                    folder.display(),
                    current.display()
                ));
                false
            }
        }
    }

    fn open_baseline(&self, report: &mut SuiteReport) -> Option<Baseline> {
        let folder = match &self.baseline {
            BaselineSource::Folder(folder) => folder.clone(),
            BaselineSource::PreviousRun => {
                let previous = self.folders.previous();
                match previous.try_exists() {
                    Ok(true) => previous,
                    Ok(false) => {
                        report.notes.push(format!(
                            "There is no earlier finished local run in {}, so every image is new",
                            previous.display()
                        ));
                        return None;
                    }
                    Err(error) => {
                        report.problems.push(format!(
                            "Could not look for the previous run in {}: {error}",
                            previous.display()
                        ));
                        return None;
                    }
                }
            }
        };
        match Baseline::open(&folder, self.folders.platform()) {
            Ok(baseline) => Some(baseline),
            Err(error) => {
                report.problems.push(error.to_string());
                None
            }
        }
    }

    fn includes(&self, screen: &Screen) -> bool {
        self.only
            .as_deref()
            .is_none_or(|text| screen.name.contains(text))
    }

    fn record_screen<'a>(
        &'a self,
        screen: &'a Screen,
        report: &'a SuiteReport,
        baseline: Option<&'a Baseline>,
    ) -> impl Iterator<Item = ImageReport> + 'a {
        SCREEN_THEMES.into_iter().map(move |(theme, theme_name)| {
            let image = image_name(screen, theme_name);
            let change = match capture_on_own_thread(&image, screen.stage, theme) {
                Ok(current) => report.folder.record(&image, &current, baseline),
                Err(message) => ImageChange::Error(message),
            };
            ImageReport {
                image,
                screen: screen.name,
                description: screen.description,
                theme: theme_name,
                change,
            }
        })
    }

    /// A run that renders nothing, or renders one image twice, must not pass as if it covered the
    /// catalogue.
    fn check_catalogue(&self, report: &mut SuiteReport) {
        let mut seen = BTreeSet::new();
        let mut repeated = BTreeSet::new();
        for image in report.images() {
            if !seen.insert(image.image.as_str()) {
                repeated.insert(image.image.clone());
            }
        }
        let mut problems: Vec<String> = repeated
            .into_iter()
            .map(|image| {
                format!("The image name {image} is used by more than one screen of the catalogue")
            })
            .collect();
        if seen.is_empty() {
            problems.push(match &self.only {
                Some(text) => format!("No image was rendered: no screen name contains \"{text}\""),
                None => "No image was rendered: the catalogue has no screens".to_owned(),
            });
        }
        report.problems.extend(problems);
    }

    /// The sheet first, so the text report can name it, and the manifest last, so a run that
    /// stopped half way has none. A write that fails is a problem the files written before it
    /// must show as well, so they are then written again.
    fn write_results(&self, report: &mut SuiteReport) {
        self.write_sheet(report);
        let text_written = self.write_report_text(report);
        let manifest = RunManifest {
            platform: report.platform.clone(),
            label: report.label.clone(),
            rendered: report.rendered.clone(),
            filter: report.filter.clone(),
        };
        if let Err(error) = manifest.write(report.folder.folder()) {
            report
                .problems
                .push(format!("Could not write the manifest: {error}"));
            self.write_sheet(report);
            if text_written {
                self.write_report_text(report);
            }
        }
    }

    /// Returns whether the text was written. A text report that cannot be written is a problem
    /// the sheet must show as well, so the sheet is then written again.
    fn write_report_text(&self, report: &mut SuiteReport) -> bool {
        let report_path = report.folder.folder().join(REPORT_FILE);
        let Err(error) = write_text(&report_path, &report.to_string()) else {
            return true;
        };
        report.problems.push(format!(
            "Could not write {}: {error}",
            report_path.display()
        ));
        if report.sheet.is_some() {
            self.write_sheet(report);
        }
        false
    }

    /// A sheet that could not be written is removed, so a half-written page or one from an
    /// earlier write of this run never shows an outdated verdict.
    fn write_sheet(&self, report: &mut SuiteReport) {
        let output = report.folder.folder().to_path_buf();
        match write_contact_sheet(report, &output) {
            Ok(path) => report.sheet = Some(path),
            Err(error) => {
                report.sheet = None;
                report.problems.push(format!(
                    "Could not write the contact sheet in {}: {error}",
                    output.display()
                ));
                remove_result_file(&output.join(CONTACT_SHEET_FILE), report);
            }
        }
    }
}

/// A screen filter was set on CI, where every run renders the whole catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenFilterOnCi;

impl fmt::Display for ScreenFilterOnCi {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "A screen filter is not allowed on CI; unset {SCREENS_VARIABLE}"
        )
    }
}

impl Error for ScreenFilterOnCi {}

/// A value that is not valid Unicode is read lossily: as a screen filter it matches no screen
/// name, so the run renders nothing and fails; as `CI` it is never empty, so it counts as set; as
/// a baseline it names a folder that is not there, so the run fails; and as a label it is shown
/// with replacement characters.
fn read_variable(name: &str) -> Option<String> {
    env::var_os(name).map(|value| value.to_string_lossy().into_owned())
}

/// Compares the real folders when both exist, so a different spelling of the same folder (case,
/// `..`, a link) is caught; a folder that is not there is compared as written.
fn is_same_or_inside(folder: &Path, run_folder: &Path) -> io::Result<bool> {
    let inside = match (real_path(folder)?, real_path(run_folder)?) {
        (Some(real_folder), Some(real_run_folder)) => real_folder.starts_with(real_run_folder),
        _ => folder.starts_with(run_folder),
    };
    Ok(inside)
}

/// The canonical path, or none when nothing is there.
fn real_path(path: &Path) -> io::Result<Option<PathBuf>> {
    match fs::canonicalize(path) {
        Ok(real) => Ok(Some(real)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(with_path(path, error)),
    }
}

/// In an unfiltered run, every baseline image without a current image belongs to a screen that
/// was removed or renamed; its baseline file is kept for the sheet.
fn keep_removed_images(baseline: &Baseline, report: &mut SuiteReport) {
    let names = match baseline.image_names() {
        Ok(names) => names,
        Err(error) => {
            report.problems.push(format!(
                "Could not list the images of the baseline {}: {error}",
                baseline.folder().display()
            ));
            return;
        }
    };
    let catalogue: BTreeSet<String> = report.images().map(|image| image.image.clone()).collect();
    for removed in names.difference(&catalogue) {
        if let Err(error) = report.folder.keep_removed(removed, baseline) {
            report.problems.push(error.to_string());
        }
        report.removed.push(removed.clone());
    }
}

/// Slint's platform, mock clock and focus are per thread, so a fresh thread makes the image
/// independent of every other image and of the order of the catalogue. A panic on that thread
/// becomes the image's failure instead of ending the run.
fn capture_on_own_thread(
    image: &str,
    stage: fn(ThemeChoice) -> Scene,
    theme: ThemeChoice,
) -> Result<RgbaImage, String> {
    let capture = thread::Builder::new()
        .name(image.to_owned())
        .spawn(move || stage(theme).capture())
        .map_err(|error| format!("Could not start a thread for {image}: {error}"))?;
    capture.join().map_err(|payload| {
        format!(
            "staging or capture panicked: {}",
            panic_text(payload.as_ref())
        )
    })
}

fn panic_text(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        return (*text).to_owned();
    }
    if let Some(text) = payload.downcast_ref::<String>() {
        return text.clone();
    }
    PANIC_WITHOUT_TEXT.to_owned()
}

/// The earlier report and contact sheet are removed before anything else, so a run that stops
/// early never leaves either to read as its own. A file that is not there is fine.
fn remove_earlier_results(folder: &Path) -> Result<(), String> {
    for file in [REPORT_FILE, CONTACT_SHEET_FILE] {
        let path = folder.join(file);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Could not remove the earlier {}: {error}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

/// A file that is not there is fine; any other failure to remove it is a problem of the run.
fn remove_result_file(path: &Path, report: &mut SuiteReport) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => report
            .problems
            .push(format!("Could not remove {}: {error}", path.display())),
    }
}

fn write_text(path: &Path, text: &str) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(path, text)
}
