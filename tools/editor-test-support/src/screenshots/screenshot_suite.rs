use std::any::Any;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::thread;

use aurora_asset_editor::settings::ThemeChoice;
use image::RgbaImage;

use super::contact_sheet::write_contact_sheet;
use super::reference_mode::ReferenceMode;
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
    /// `CI`, and the filter from `AURORA_UI_SCREENS`. Panics on a mode the run must refuse.
    pub fn from_environment() -> ScreenshotSuite {
        let mode = ReferenceMode::from_environment().unwrap_or_else(|error| panic!("{error}"));
        let only = env::var_os(SCREENS_VARIABLE)
            .map(|value| value.to_string_lossy().into_owned())
            .filter(|text| !text.is_empty());
        ScreenshotSuite::new(ReferenceStore::for_workspace(), mode, only)
    }

    pub fn run(&self, areas: &[ScreenArea]) -> SuiteReport {
        let mut report = SuiteReport {
            store: self.store.clone(),
            areas: Vec::new(),
            orphans: Vec::new(),
            removed_orphans: Vec::new(),
            filter: self.only.clone(),
            problems: Vec::new(),
            sheet: None,
        };
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
    fn write_results(&self, report: &mut SuiteReport) {
        let output = self.store.output();
        match write_contact_sheet(report, output) {
            Ok(path) => report.sheet = Some(path),
            Err(error) => report.problems.push(format!(
                "Could not write the contact sheet in {}: {error}",
                output.display()
            )),
        }
        let report_path = output.join(REPORT_FILE);
        if let Err(error) = write_text(&report_path, &report.to_string()) {
            report.problems.push(format!(
                "Could not write {}: {error}",
                report_path.display()
            ));
        }
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

fn write_text(path: &Path, text: &str) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(path, text)
}
