use std::fmt;
use std::path::{MAIN_SEPARATOR, Path};
use std::sync::OnceLock;

use aurora_ui::MainWindow;
use i_slint_backend_testing::{ElementHandle, ElementRoot};
use ttf_parser::Face;

use crate::STAGED_CONSOLE_HOST;

/// Texts that differ between machines: the local host's name and the OS's wording of an `io::Error`.
/// Addresses are checked separately: only the staged console's may appear.
pub const FORBIDDEN_SCREEN_TEXT: [&str; 2] = ["localhost", "os error"];

/// The two faces the UI draws its text with; `main_window.slint` imports the same files.
const INTER_FONTS: [&[u8]; 2] = [
    include_bytes!("../../../../crates/aurora-ui/ui/fonts/Inter-Regular.ttf"),
    include_bytes!("../../../../crates/aurora-ui/ui/fonts/Inter-SemiBold.ttf"),
];
/// Without element debug info the walk finds no elements, and a check that sees nothing must not pass.
const NO_SCREEN_TEXT: &str =
    "the screen has no text to check: element debug info is missing; run in the debug profile";

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

/// A folder whose path differs between machines, so no screen may show it.
struct MachineFolder {
    name: &'static str,
    /// Without its trailing separator, so a text naming the folder itself matches too.
    path: String,
}

impl MachineFolder {
    fn new(name: &'static str, path: &Path) -> MachineFolder {
        let path = path.display().to_string();
        let path = match path.strip_suffix(MAIN_SEPARATOR) {
            Some(trimmed) if !trimmed.is_empty() => trimmed.to_owned(),
            _ => path,
        };
        MachineFolder { name, path }
    }

    /// The temp folder, the workspace the tests were built in, and the home folder.
    fn all() -> Vec<MachineFolder> {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("editor-test-support sits two folders below the workspace root");
        let mut folders = vec![
            MachineFolder::new("temp folder", &std::env::temp_dir()),
            MachineFolder::new("workspace folder", workspace),
        ];
        if let Some(home) = std::env::home_dir() {
            folders.push(MachineFolder::new("home folder", &home));
        }
        folders
    }

    /// Windows paths are case-insensitive, so the app may show the folder in another case.
    fn is_in(&self, text: &str) -> bool {
        if cfg!(windows) {
            text.to_lowercase().contains(&self.path.to_lowercase())
        } else {
            text.contains(&self.path)
        }
    }
}

/// Checks every text of the window's elements (accessible label, value, placeholder and description) for
/// machine-specific content: a forbidden text, a machine folder, or a character the bundled Inter lacks.
pub fn check_screen_text(window: &MainWindow) -> Result<(), ScreenTextProblem> {
    let problems = problems_on_screen(&screen_texts(window), &MachineFolder::all());
    if problems.is_empty() {
        Ok(())
    } else {
        Err(ScreenTextProblem { problems })
    }
}

fn problems_on_screen(texts: &[String], folders: &[MachineFolder]) -> Vec<String> {
    if texts.is_empty() {
        return vec![NO_SCREEN_TEXT.to_owned()];
    }
    texts
        .iter()
        .flat_map(|text| problems_in(text, folders))
        .collect()
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

fn problems_in(text: &str, folders: &[MachineFolder]) -> Vec<String> {
    let mut problems: Vec<String> = FORBIDDEN_SCREEN_TEXT
        .iter()
        .filter(|forbidden| text.contains(*forbidden))
        .map(|forbidden| format!("\"{text}\" contains \"{forbidden}\""))
        .collect();
    for address in addresses_in(text)
        .into_iter()
        .filter(|address| *address != STAGED_CONSOLE_HOST)
    {
        problems.push(format!(
            "\"{text}\" shows the address {address}; screens show only the staged console {STAGED_CONSOLE_HOST}"
        ));
    }
    for folder in folders.iter().filter(|folder| folder.is_in(text)) {
        problems.push(format!(
            "\"{text}\" contains the {} \"{}\"",
            folder.name, folder.path
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

/// The IPv4 addresses in `text`: four groups of 1 to 3 digits (each at most 255) joined by single dots, with
/// no digit or dot directly before or after.
fn addresses_in(text: &str) -> Vec<&str> {
    text.split(|character: char| !character.is_ascii_digit() && character != '.')
        .filter(|run| is_address(run))
        .collect()
}

fn is_address(run: &str) -> bool {
    let groups: Vec<&str> = run.split('.').collect();
    groups.len() == 4
        && groups.iter().all(|group| {
            (1..=3).contains(&group.len())
                // `u8::parse` accepts a leading '+', so the digits are checked on their own.
                && group.bytes().all(|byte| byte.is_ascii_digit())
                && group.parse::<u8>().is_ok()
        })
}

/// The characters of `text` that are missing from either Inter face, each named once. Control characters
/// (tab, newline) are layout, not glyphs; every other character, unusual spaces included, needs a glyph.
fn chars_missing_from_inter(text: &str) -> Vec<char> {
    let faces = inter_faces();
    let mut missing = Vec::new();
    for character in text.chars() {
        let in_both_faces = faces
            .iter()
            .all(|face| face.glyph_index(character).is_some());
        if !character.is_control() && !in_both_faces && !missing.contains(&character) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_without_text_fails_the_check() {
        assert_eq!(problems_on_screen(&[], &[]), vec![NO_SCREEN_TEXT]);
    }

    #[test]
    fn addresses_are_found_in_text() {
        assert_eq!(addresses_in("192.168.1.50:21 · xboxftp"), ["192.168.1.50"]);
        assert!(addresses_in("Slint 1.18.1").is_empty());
        assert!(addresses_in("1.2.3.4.5").is_empty());
        assert!(addresses_in("999.1.1.1").is_empty());
        assert!(addresses_in("192.168.1.").is_empty());
        assert!(addresses_in("0001.1.1.1").is_empty());
        assert!(addresses_in("1.2.3.4.").is_empty());
        assert_eq!(
            addresses_in("192.168.1.50 and 10.0.0.1"),
            ["192.168.1.50", "10.0.0.1"]
        );
        assert!(!is_address("1.+2.3.4"));
        assert!(!is_address("+1.2.3.4"));
    }

    #[test]
    fn inter_has_a_plain_space() {
        assert!(chars_missing_from_inter("Halo 3").is_empty());
    }
}
