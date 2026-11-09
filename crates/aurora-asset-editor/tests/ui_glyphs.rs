use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const FONTS: [&str; 2] = ["Inter-Regular.ttf", "Inter-SemiBold.ttf"];
/// Separators the UI copy uses between parts of a line, checked even if no source names them yet.
const COPY_SEPARATORS: [char; 2] = ['·', '…'];

fn ui_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../aurora-ui/ui")
}

fn files_with_extension(folder: &Path, extension: &str, found: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(folder).expect("the source folder is readable");
    for entry in entries {
        let path = entry.expect("the folder entry is readable").path();
        if path.is_dir() {
            files_with_extension(&path, extension, found);
        } else if path.extension().is_some_and(|found| found == extension) {
            found.push(path);
        }
    }
}

fn string_literal_chars(source: &str, chars: &mut BTreeSet<char>) {
    let mut in_literal = false;
    let mut escaped = false;
    for character in source.chars() {
        if !in_literal {
            in_literal = character == '"';
        } else if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            in_literal = false;
        } else {
            chars.insert(character);
        }
    }
}

/// Every character a user can see: the strings of the `.slint` files and of the app's Rust sources (status
/// texts, dialog titles and texts).
fn ui_copy_chars() -> BTreeSet<char> {
    let mut sources = Vec::new();
    files_with_extension(&ui_folder(), "slint", &mut sources);
    files_with_extension(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        "rs",
        &mut sources,
    );
    let mut chars = BTreeSet::from(COPY_SEPARATORS);
    for path in sources {
        let source = fs::read_to_string(&path).expect("the source file is UTF-8");
        string_literal_chars(&source, &mut chars);
    }
    chars.retain(|character| !character.is_control());
    chars
}

#[test]
fn bundled_inter_has_every_glyph_of_the_ui_copy() {
    let chars = ui_copy_chars();
    assert!(chars.contains(&'·') && chars.contains(&'…') && chars.contains(&'A'));
    for font in FONTS {
        let bytes = fs::read(ui_folder().join("fonts").join(font)).expect("the font is bundled");
        let face = ttf_parser::Face::parse(&bytes, 0).expect("the font parses");
        let missing: Vec<char> = chars
            .iter()
            .copied()
            .filter(|character| face.glyph_index(*character).is_none())
            .collect();
        assert!(missing.is_empty(), "{font} lacks {missing:?}");
    }
}
