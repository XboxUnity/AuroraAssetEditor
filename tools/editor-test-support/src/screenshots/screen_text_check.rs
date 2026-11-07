use std::fmt;
use std::path::MAIN_SEPARATOR;
use std::sync::OnceLock;

use aurora_ui::MainWindow;
use i_slint_backend_testing::{ElementHandle, ElementRoot};
use ttf_parser::Face;

/// Texts that differ between machines: the test servers' address and the OS's wording of an `io::Error`.
pub const FORBIDDEN_SCREEN_TEXT: [&str; 3] = ["127.0.0.1", "localhost", "os error"];

/// The two faces the UI draws its text with; `main_window.slint` imports the same files.
const INTER_FONTS: [&[u8]; 2] = [
    include_bytes!("../../../../crates/aurora-ui/ui/fonts/Inter-Regular.ttf"),
    include_bytes!("../../../../crates/aurora-ui/ui/fonts/Inter-SemiBold.ttf"),
];

/// Texts on a screen that would make its capture differ on another machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenTextProblem {
    pub problems: Vec<String>,
}

impl fmt::Display for ScreenTextProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the screen shows text that would differ on another machine:"
        )?;
        for problem in &self.problems {
            write!(formatter, "\n  {problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ScreenTextProblem {}

/// Checks every text of the window's elements (accessible label, value, placeholder and description) for
/// machine-specific content: a forbidden text, the temp folder, or a character the bundled Inter lacks.
pub fn check_screen_text(window: &MainWindow) -> Result<(), ScreenTextProblem> {
    let temp_folder = temp_folder_text();
    let problems: Vec<String> = screen_texts(window)
        .iter()
        .flat_map(|text| problems_in(text, &temp_folder))
        .collect();
    if problems.is_empty() {
        Ok(())
    } else {
        Err(ScreenTextProblem { problems })
    }
}

/// Every distinct non-empty text of the window, in tree order.
fn screen_texts(window: &MainWindow) -> Vec<String> {
    let mut texts: Vec<String> = Vec::new();
    for element in window.root_element().query_descendants().find_all() {
        for text in element_texts(&element) {
            if !text.is_empty() && !texts.contains(&text) {
                texts.push(text);
            }
        }
    }
    texts
}

fn element_texts(element: &ElementHandle) -> impl Iterator<Item = String> {
    [
        element.accessible_label(),
        element.accessible_value(),
        element.accessible_placeholder_text(),
        element.accessible_description(),
    ]
    .into_iter()
    .flatten()
    .map(|text| text.to_string())
}

fn problems_in(text: &str, temp_folder: &str) -> Vec<String> {
    let mut problems: Vec<String> = FORBIDDEN_SCREEN_TEXT
        .iter()
        .filter(|forbidden| text.contains(*forbidden))
        .map(|forbidden| format!("\"{text}\" contains \"{forbidden}\""))
        .collect();
    if contains_folder(text, temp_folder) {
        problems.push(format!(
            "\"{text}\" contains the temp folder \"{temp_folder}\""
        ));
    }
    let missing = chars_missing_from_inter(text);
    if !missing.is_empty() {
        problems.push(format!(
            "\"{text}\" has {missing:?}, which the bundled Inter lacks, so a system font would draw them"
        ));
    }
    problems
}

/// The temp folder without its trailing separator, so a text naming the folder itself matches too.
fn temp_folder_text() -> String {
    let folder = std::env::temp_dir().display().to_string();
    match folder.strip_suffix(MAIN_SEPARATOR) {
        Some(trimmed) if !trimmed.is_empty() => trimmed.to_owned(),
        _ => folder,
    }
}

/// Windows paths are case-insensitive, so the app may show the folder in another case.
fn contains_folder(text: &str, folder: &str) -> bool {
    if cfg!(windows) {
        text.to_lowercase().contains(&folder.to_lowercase())
    } else {
        text.contains(folder)
    }
}

/// The visible characters of `text` that are missing from either Inter face, each named once.
fn chars_missing_from_inter(text: &str) -> Vec<char> {
    let faces = inter_faces();
    let mut missing = Vec::new();
    for character in text.chars() {
        let drawn = !character.is_whitespace() && !character.is_control();
        let in_both_faces = faces
            .iter()
            .all(|face| face.glyph_index(character).is_some());
        if drawn && !in_both_faces && !missing.contains(&character) {
            missing.push(character);
        }
    }
    missing
}

fn inter_faces() -> &'static [Face<'static>; 2] {
    static FACES: OnceLock<[Face<'static>; 2]> = OnceLock::new();
    FACES.get_or_init(|| {
        INTER_FONTS.map(|font| Face::parse(font, 0).expect("the bundled Inter fonts parse"))
    })
}
