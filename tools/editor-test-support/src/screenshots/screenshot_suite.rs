use std::any::Any;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::thread;

use aurora_asset_editor::settings::ThemeChoice;
use image::RgbaImage;

use super::contact_sheet::{CONTACT_SHEET_FILE, write_contact_sheet};
use super::reference_mode::{CI_VARIABLE, ReferenceMode, read_variable};
use super::reference_store::{ImageStatus, ReferenceStore};
use super::scene::Scene;
use super::screen::{SCREEN_THEMES, Screen, ScreenArea, image_name};
use super::suite_report::{AreaReport, ImageReport, SuiteReport};

/// Limits a run to the screens whose name contains its value.
pub const SCREENS_VARIABLE: &str = "AURORA_UI_SCREENS";
/// The run report as plain text, next to the contact sheet: the same text a failing test prints.
pub const REPORT_FILE: &str = "report.txt";

const PANIC_WITHOUT_TEXT: &str = "a panic without a message";

/// Captures every screen of a catalogue in both themes and checks each image against its
/// approved reference.
pub struct ScreenshotSuite {
    store: ReferenceStore,
    mode: ReferenceMode,
    only: Option<String>,
}

impl ScreenshotSuite {
    /// `only` limits the run to the screens whose name contains it; such a run cannot tell
    /// which references lost their screen, so it skips the orphan check.
    pub fn new(
        store: ReferenceStore,
        mode: ReferenceMode,
        only: Option<String>,
    ) -> ScreenshotSuite {
        ScreenshotSuite { store, mode, only }
    }

    /// The workspace's references and output folder, the mode from `AURORA_UI_REFERENCES` and
    /// `CI`, and the filter from `AURORA_UI_SCREENS`. Panics on a mode or a filter the run must
    /// refuse, naming why.
    pub fn from_environment() -> ScreenshotSuite {
        let mode = ReferenceMode::from_environment().unwrap_or_else(|error| panic!("{error}"));
        let only = ScreenshotSuite::filter_from_values(
            read_variable(SCREENS_VARIABLE).as_deref(),
            read_variable(CI_VARIABLE).as_deref(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        ScreenshotSuite::new(ReferenceStore::for_workspace(), mode, only)
    }

    /// An unset or empty `screens` means every screen. A filter is refused when `ci` is set to
    /// anything non-empty: CI must always check the whole catalogue.
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

    pub fn run(&self, areas: &[ScreenArea]) -> SuiteReport {
        let mut report = SuiteReport {
            store: self.store.clone(),
            mode: self.mode,
            areas: Vec::new(),
            orphans: Vec::new(),
            removed_orphans: Vec::new(),
            filter: self.only.clone(),
            problems: Vec::new(),
            sheet: None,
        };
        self.remove_previous_results(&mut report);
        for area in areas {
            let images: Vec<ImageReport> = area
                .screens
                .iter()
                .filter(|screen| self.includes(screen))
                .flat_map(|screen| self.check_screen(screen))
                .collect();
            if !images.is_empty() {
                report.areas.push(AreaReport {
                    title: area.title,
                    images,
                });
            }
        }
        self.check_catalogue(&mut report);
        if self.only.is_none() {
            let catalogue: BTreeSet<String> =
                report.images().map(|image| image.image.clone()).collect();
            self.handle_orphans(&catalogue, &mut report);
            self.clear_retired_outputs(&catalogue, &mut report);
        }
        self.write_results(&mut report);
        report
    }

    fn includes(&self, screen: &Screen) -> bool {
        self.only
            .as_deref()
            .is_none_or(|text| screen.name.contains(text))
    }

    fn check_screen<'a>(&'a self, screen: &'a Screen) -> impl Iterator<Item = ImageReport> + 'a {
        SCREEN_THEMES
            .into_iter()
            .map(move |(theme, theme_name)| self.check_image(screen, theme, theme_name))
    }

    fn check_image(
        &self,
        screen: &Screen,
        theme: ThemeChoice,
        theme_name: &'static str,
    ) -> ImageReport {
        let image = image_name(screen, theme_name);
        let status = self.image_status(&image, screen.stage, theme);
        ImageReport {
            image,
            screen: screen.name,
            description: screen.description,
            theme: theme_name,
            status,
        }
    }

    /// The earlier outputs go first, so an image whose staging panics leaves nothing old beside
    /// its error.
    fn image_status(
        &self,
        image: &str,
        stage: fn(ThemeChoice) -> Scene,
        theme: ThemeChoice,
    ) -> ImageStatus {
        if let Err(error) = self.store.clear_outputs(image) {
            return ImageStatus::Error(format!(
                "The earlier outputs of {image} could not be removed: {error}"
            ));
        }
        match capture_on_own_thread(image, stage, theme) {
            Ok(actual) => self.store.check(image, &actual, self.mode),
            Err(message) => ImageStatus::Error(message),
        }
    }

    /// A run that checks nothing, or checks one image twice, must not pass as if it covered the
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
                Some(text) => format!("No image was checked: no screen name contains \"{text}\""),
                None => "No image was checked: the catalogue has no screens".to_owned(),
            });
        }
        report.problems.extend(problems);
    }

    /// References whose image is not in the catalogue: their screen was removed or renamed.
    fn handle_orphans(&self, catalogue: &BTreeSet<String>, report: &mut SuiteReport) {
        let references = match self.store.reference_names() {
            Ok(names) => names,
            Err(error) => {
                report.problems.push(format!(
                    "Could not list the references in {}: {error}",
                    self.store.references().display()
                ));
                return;
            }
        };
        for orphan in references.difference(catalogue) {
            match self.mode {
                ReferenceMode::Check => report.orphans.push(orphan.clone()),
                ReferenceMode::Update => match self.store.remove_reference(orphan) {
                    Ok(()) => report.removed_orphans.push(orphan.clone()),
                    Err(error) => {
                        report.orphans.push(orphan.clone());
                        report.problems.push(format!(
                            "Could not remove the orphan reference {orphan}: {error}"
                        ));
                    }
                },
            }
        }
    }

    /// Outputs of images no longer in the catalogue, so neither the output folder nor the CI zip
    /// shows a screen that was removed.
    fn clear_retired_outputs(&self, catalogue: &BTreeSet<String>, report: &mut SuiteReport) {
        let outputs = match self.store.output_names() {
            Ok(names) => names,
            Err(error) => {
                report.problems.push(format!(
                    "Could not list the outputs in {}: {error}",
                    self.store.output().display()
                ));
                return;
            }
        };
        for retired in outputs.difference(catalogue) {
            if let Err(error) = self.store.clear_outputs(retired) {
                report.problems.push(format!(
                    "Could not remove the outputs of {retired}, which has no screen any more: {error}"
                ));
            }
        }
    }

    /// The contact sheet first, so the text report can name it.
    /// A text report that cannot be written is a problem the sheet must show as well, so the
    /// sheet is then written again.
    fn write_results(&self, report: &mut SuiteReport) {
        self.write_sheet(report);
        let report_path = self.store.output().join(REPORT_FILE);
        if let Err(error) = write_text(&report_path, &report.to_string()) {
            report.problems.push(format!(
                "Could not write {}: {error}",
                report_path.display()
            ));
            if report.sheet.is_some() {
                self.write_sheet(report);
            }
        }
    }

    /// A sheet that could not be written is removed, so a half-written page or one from an
    /// earlier write of this run never shows an outdated verdict.
    fn write_sheet(&self, report: &mut SuiteReport) {
        let output = self.store.output();
        match write_contact_sheet(report, output) {
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

    /// The previous run's sheet and text report go first, so a run whose writes fail never
    /// leaves an older verdict on disk.
    fn remove_previous_results(&self, report: &mut SuiteReport) {
        for file in [CONTACT_SHEET_FILE, REPORT_FILE] {
            remove_result_file(&self.store.output().join(file), report);
        }
    }
}

/// A screen filter was set on CI, where every run checks the whole catalogue.
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
