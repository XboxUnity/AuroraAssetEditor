//! Directory listing lines (RFC 3659 MLSD, Unix `ls -l`, MS-DOS `dir`) reduced to names and sizes.

use std::sync::LazyLock;

use regex::{Captures, Regex};

const CONTINUATION_STARTS: [char; 2] = [' ', '\t'];
const DOT_NAMES: [&str; 2] = [".", ".."];
const PATH_PREFIXES: [&str; 3] = ["/", "./", "../"];
const PATH_SEPARATOR: char = '/';

const MLSD_NAME_SEPARATOR: &str = "; ";
const MLSD_ENTRY_TYPES: [&str; 4] = ["dir", "pdir", "cdir", "file"];

const UNIX_ENTRY_TYPES: [char; 4] = ['d', '-', 's', 'l'];
const UNIX_DIRECTORY: char = 'd';
const UNIX_SYMLINK: char = 'l';
const SYMLINK_ARROW: &str = " -> ";

/// Sizes beyond the signed 64-bit range read as unknown instead of rejecting the line.
const LARGEST_SIZE: u64 = i64::MAX.unsigned_abs();

static MLSD_TYPE_FACT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)type=(?<value>[^;]+);").expect("the MLSD type pattern is a valid constant")
});

static MLSD_SIZE_FACT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)size=(?<value>[0-9]+);").expect("the MLSD size pattern is a valid constant")
});

/// Permissions, link count, owner, group, size, `Mon DD hh:mm` or `Mon DD YYYY`, then one
/// whitespace character and the name.
static UNIX_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?<permissions>\S+)\s+(?<links>[0-9]+)\s+(?<owner>\S+)\s+(?<group>\S+)\s+",
        r"(?<size>[0-9]+)\s+(?<month>\w+)\s+(?<day>[0-9]+)\s+(?<time_or_year>[0-9]+:[0-9]+|[0-9]+)",
        r"\s(?<name>.*)$",
    ))
    .expect("the Unix listing pattern is a valid constant")
});

static DOS_DIRECTORY_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[0-9]+-[0-9]+-[0-9]+\s+[0-9]+:[0-9]+[a-z]+\s+<DIR>\s+(?<name>.*)$")
        .expect("the DOS directory pattern is a valid constant")
});

static DOS_FILE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[0-9]+-[0-9]+-[0-9]+\s+[0-9]+:[0-9]+[a-z]+\s+(?<size>[0-9]+)\s+(?<name>.*)$")
        .expect("the DOS file pattern is a valid constant")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListingEntry {
    pub(crate) name: String,
    pub(crate) size: Option<u64>,
}

/// Entries in listing order; lines no format recognises are left out.
pub(crate) fn parse_listing(lines: &[String]) -> Vec<ListingEntry> {
    let non_empty_lines: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|line| !line.is_empty())
        .collect();
    merge_continuations(&non_empty_lines)
        .iter()
        .filter_map(|line| parse_entry_line(line))
        .collect()
}

/// Compares names with ASCII case folding, as FATX lookups ignore case.
pub(crate) fn find_entry<'a>(entries: &'a [ListingEntry], name: &str) -> Option<&'a ListingEntry> {
    entries
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(name))
}

/// Some servers wrap a long name onto an indented line; at most one such line joins each entry.
fn merge_continuations(lines: &[&str]) -> Vec<String> {
    let mut merged = Vec::with_capacity(lines.len());
    let mut remaining = lines.iter().peekable();
    while let Some(line) = remaining.next() {
        let mut entry_line = (*line).to_owned();
        if let Some(continuation) = remaining.next_if(|next| next.starts_with(CONTINUATION_STARTS))
        {
            entry_line.push_str(continuation);
        }
        merged.push(entry_line);
    }
    merged
}

fn parse_entry_line(line: &str) -> Option<ListingEntry> {
    let entry = parse_mlsd_line(line)
        .or_else(|| parse_unix_line(line))
        .or_else(|| parse_dos_line(line))?;
    let name = last_path_component(&entry.name);
    if DOT_NAMES.contains(&name) {
        return None;
    }
    Some(ListingEntry {
        name: name.to_owned(),
        size: entry.size,
    })
}

/// `type=file;size=2048;modify=20261107120000; Content.db`. The facts end at the first `; `, so a
/// name that looks like a fact is never read as one.
fn parse_mlsd_line(line: &str) -> Option<ListingEntry> {
    let (facts_end, name) = line
        .split_once(MLSD_NAME_SEPARATOR)
        .map(|(facts, name)| (facts.len() + 1, name))?;
    let facts = &line[..facts_end];
    let entry_type = group(&MLSD_TYPE_FACT.captures(facts)?, "value")?.to_ascii_lowercase();
    if !MLSD_ENTRY_TYPES.contains(&entry_type.as_str()) {
        return None;
    }
    let size = MLSD_SIZE_FACT
        .captures(facts)
        .and_then(|fact| group(&fact, "value"))
        .and_then(parse_size);
    Some(ListingEntry {
        name: name.to_owned(),
        size,
    })
}

/// `-rw-r--r--   1 root  root      65536 Nov 07 12:00 GC4D5307E6.asset`
fn parse_unix_line(line: &str) -> Option<ListingEntry> {
    let entry_type = line
        .chars()
        .next()
        .filter(|first| UNIX_ENTRY_TYPES.contains(first))?;
    let fields = UNIX_LINE.captures(line)?;
    let listed_name = group(&fields, "name")?;
    let name = match entry_type {
        UNIX_SYMLINK => listed_name.split_once(SYMLINK_ARROW)?.0,
        UNIX_DIRECTORY if DOT_NAMES.contains(&listed_name) => return None,
        _ => listed_name,
    };
    if name.is_empty() {
        return None;
    }
    Some(ListingEntry {
        name: name.to_owned(),
        size: group(&fields, "size").and_then(parse_size),
    })
}

/// `11-07-26  12:00PM       <DIR>          4D5307E6_00000001` or
/// `11-07-26  12:00PM                 2048 GC4D5307E6.asset`
fn parse_dos_line(line: &str) -> Option<ListingEntry> {
    if let Some(fields) = DOS_DIRECTORY_LINE.captures(line) {
        return Some(ListingEntry {
            name: group(&fields, "name")?.to_owned(),
            size: None,
        });
    }
    let fields = DOS_FILE_LINE.captures(line)?;
    Some(ListingEntry {
        name: group(&fields, "name")?.to_owned(),
        size: group(&fields, "size").and_then(parse_size),
    })
}

/// Only names that start like a path are shortened, and never to an empty name.
fn last_path_component(name: &str) -> &str {
    if !PATH_PREFIXES.iter().any(|prefix| name.starts_with(prefix)) {
        return name;
    }
    match name.rsplit_once(PATH_SEPARATOR) {
        Some((_, last)) if !last.is_empty() => last,
        _ => name,
    }
}

fn parse_size(digits: &str) -> Option<u64> {
    digits
        .parse::<u64>()
        .ok()
        .filter(|&size| size <= LARGEST_SIZE)
}

fn group<'line>(captures: &Captures<'line>, name: &str) -> Option<&'line str> {
    captures.name(name).map(|matched| matched.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ftp::terminated_lines;

    fn parse(lines: &[&str]) -> Vec<ListingEntry> {
        let lines: Vec<String> = lines.iter().map(|line| line.to_string()).collect();
        parse_listing(&lines)
    }

    fn entry(name: &str, size: Option<u64>) -> ListingEntry {
        ListingEntry {
            name: name.to_owned(),
            size,
        }
    }

    fn parse_body(body: &str) -> Vec<ListingEntry> {
        let lines: Vec<String> = terminated_lines(body.as_bytes()).collect();
        parse_listing(&lines)
    }

    #[test]
    fn parses_unix_line() {
        assert_eq!(
            parse(&["-rw-r--r--   1 root  root      65536 Nov 07 12:00 GC4D5307E6.asset"]),
            [entry("GC4D5307E6.asset", Some(65536))]
        );
    }

    #[test]
    fn parses_unix_name_with_spaces_and_year() {
        assert_eq!(
            parse(&["-rw-r--r-- 1 a b 10 Nov 07  2025 My Game.asset"]),
            [entry("My Game.asset", Some(10))]
        );
    }

    #[test]
    fn parses_unix_symlink() {
        assert_eq!(
            parse(&["lrwxrwxrwx 1 a b 7 Nov 07 12:00 link -> target"]),
            [entry("link", Some(7))]
        );
    }

    #[test]
    fn parses_dos_lines() {
        assert_eq!(
            parse(&[
                "11-07-26  12:00PM                 2048 GC4D5307E6.asset",
                "11-07-26  12:00PM       <DIR>          4D5307E6_00000001",
            ]),
            [
                entry("GC4D5307E6.asset", Some(2048)),
                entry("4D5307E6_00000001", None)
            ]
        );
    }

    #[test]
    fn parses_mlsd_line() {
        assert_eq!(
            parse(&[
                "type=cdir;modify=20261107120000; .",
                "type=file;size=2048;modify=20261107120000; Content.db",
            ]),
            [entry("Content.db", Some(2048))]
        );
    }

    #[test]
    fn mlsd_line_of_unknown_type_is_rejected() {
        assert_eq!(parse(&["type=OS.unix=slink;size=7; link"]), []);
    }

    #[test]
    fn merges_continuation_lines() {
        assert_eq!(
            parse(&["-rw-r--r-- 1 a b 10 Nov 07 12:00 Long", " Name.asset"]),
            [entry("Long Name.asset", Some(10))]
        );
        assert_eq!(
            parse(&["-rw-r--r-- 1 a b 10 Nov 07 12:00 Long", "\tName.asset"]),
            [entry("Long\tName.asset", Some(10))]
        );
    }

    #[test]
    fn only_the_next_line_is_merged() {
        assert_eq!(
            parse(&["-rw-r--r-- 1 a b 10 Nov 07 12:00 Long", " Name", " Tail"]),
            [entry("Long Name", Some(10))]
        );
    }

    #[test]
    fn skips_garbage_and_empty_lines() {
        assert_eq!(
            parse(&[
                "",
                "total 8",
                "garbage",
                "-rw-r--r-- 1 a b 10 Nov 07 12:00 GC1.asset",
            ]),
            [entry("GC1.asset", Some(10))]
        );
    }

    #[test]
    fn drops_dot_directories() {
        assert_eq!(
            parse(&[
                "drwxr-xr-x 2 a b 0 Nov 07 12:00 .",
                "drwxr-xr-x 2 a b 0 Nov 07 12:00 ..",
            ]),
            []
        );
    }

    #[test]
    fn absolute_names_keep_last_component() {
        assert_eq!(
            parse(&[
                "-rw-r--r-- 1 a b 10 Nov 07 12:00 /Game/Data/GC1.asset",
                "type=file;size=20; ./GC2.asset",
                "type=file;size=30; ../GC3.asset",
                "type=dir; /Game/Data/",
            ]),
            [
                entry("GC1.asset", Some(10)),
                entry("GC2.asset", Some(20)),
                entry("GC3.asset", Some(30)),
                entry("/Game/Data/", None)
            ]
        );
    }

    #[test]
    fn find_entry_ignores_case() {
        let entries = [entry("content.db", Some(1)), entry("Content.db", Some(2))];
        assert_eq!(find_entry(&entries, "Content.db"), Some(&entries[0]));
        assert_eq!(find_entry(&entries, "Other.db"), None);
    }

    #[test]
    fn every_listing_pattern_compiles() {
        for pattern in [
            &MLSD_TYPE_FACT,
            &MLSD_SIZE_FACT,
            &UNIX_LINE,
            &DOS_DIRECTORY_LINE,
            &DOS_FILE_LINE,
        ] {
            LazyLock::force(pattern);
        }
    }

    #[test]
    fn sizes_beyond_the_signed_64_bit_range_are_unknown() {
        assert_eq!(
            parse(&[
                "-rw-r--r-- 1 a b 9223372036854775807 Nov 07 12:00 largest",
                "-rw-r--r-- 1 a b 9223372036854775808 Nov 07 12:00 beyond",
                "-rw-r--r-- 1 a b 0 Nov 07 12:00 empty",
                "type=file;size=99999999999999999999; mlsd",
                "11-07-26  12:00PM  9223372036854775808 dos",
            ]),
            [
                entry("largest", Some(9_223_372_036_854_775_807)),
                entry("beyond", None),
                entry("empty", Some(0)),
                entry("mlsd", None),
                entry("dos", None),
            ]
        );
    }

    #[test]
    fn unix_type_character_must_be_known_and_lower_case() {
        for first in ["c", "b", "p", "D", "L", " ", "1"] {
            let line = format!("{first}rw-r--r-- 1 a b 10 Nov 07 12:00 name");
            assert_eq!(parse(&[&line]), [], "{line:?}");
        }
        assert_eq!(
            parse(&["srw-r--r-- 1 a b 0 Nov 07 12:00 socket"]),
            [entry("socket", Some(0))]
        );
    }

    #[test]
    fn unix_name_is_everything_after_one_separator() {
        assert_eq!(
            parse(&["-rw-r--r-- 1 a b 10 Nov 07 12:00  leading  space "]),
            [entry(" leading  space ", Some(10))]
        );
        assert_eq!(parse(&["-rw-r--r-- 1 a b 10 Nov 07 12:00 "]), []);
    }

    #[test]
    fn unix_directory_keeps_its_listed_size() {
        assert_eq!(
            parse(&["drwxr-xr-x 2 a b 4096 Nov 07 12:00 Data"]),
            [entry("Data", Some(4096))]
        );
    }

    #[test]
    fn symlink_needs_an_arrow_and_keeps_the_link_name() {
        assert_eq!(parse(&["lrwxrwxrwx 1 a b 7 Nov 07 12:00 link"]), []);
        assert_eq!(
            parse(&["lrwxrwxrwx 1 a b 7 Nov 07 12:00 /Game/link -> /a -> b"]),
            [entry("link", Some(7))]
        );
    }

    #[test]
    fn dos_markers_ignore_case() {
        assert_eq!(
            parse(&[
                "11-07-26  09:15am       <dir>          Folder",
                "11-07-26  09:15pm  5   File Name.bin",
            ]),
            [entry("Folder", None), entry("File Name.bin", Some(5))]
        );
    }

    #[test]
    fn mlsd_facts_ignore_case_and_order() {
        assert_eq!(
            parse(&["Modify=20261107120000;Size=12;Type=FILE; Two Words.asset"]),
            [entry("Two Words.asset", Some(12))]
        );
        assert_eq!(
            parse(&["type=dir;modify=20261107120000; Folder"]),
            [entry("Folder", None)]
        );
        assert_eq!(parse(&["type=file;size=5;"]), []);
    }

    #[test]
    fn mlsd_facts_are_read_from_the_facts_part_only() {
        assert_eq!(
            parse(&[
                "type=file; size=3;x",
                "type=file;size=7; type=dir;y",
                "size=3; type=file;x",
            ]),
            [entry("size=3;x", None), entry("type=dir;y", Some(7))]
        );
    }

    #[test]
    fn mlsd_and_dos_accept_an_empty_name() {
        assert_eq!(parse(&["type=file; "]), [entry("", None)]);
        assert_eq!(
            parse(&["11-07-26  12:00PM       <DIR>   "]),
            [entry("", None)]
        );
    }

    #[test]
    fn whitespace_only_lines_are_kept_for_merging() {
        assert_eq!(
            parse(&["-rw-r--r-- 1 a b 10 Nov 07 12:00 Name", "   "]),
            [entry("Name   ", Some(10))]
        );
        assert_eq!(parse(&["   "]), []);
    }

    #[test]
    fn only_path_like_names_are_shortened() {
        assert_eq!(
            parse(&[
                "type=file;size=1; a/b",
                "type=dir; /",
                "type=file;size=2; /Game/.",
            ]),
            [entry("a/b", Some(1)), entry("/", None)]
        );
    }

    #[test]
    fn find_entry_folds_ascii_case_only() {
        let entries = [entry("Ärger.asset", Some(1))];
        assert_eq!(find_entry(&entries, "äRGER.ASSET"), None);
        assert_eq!(find_entry(&entries, "Ärger.ASSET"), Some(&entries[0]));
    }

    #[test]
    fn aurora_asset_folder_listing() {
        let body = concat!(
            "drwxrwxrwx   1 root root             0 Jan 05 2019 4D5307E6_00000012\r\n",
            "-rw-rw-rw-   1 root root       1234567 Nov 03 14:05 GC4D5307E6.asset\r\n",
            "-rwxrwxrwx   1 root root    4294967295 Jun 30 23:59 Content.db\r\n",
            "-rw-rw-rw-   1 root root            24 Jan 05 2019 My Saved Game.bin\r\n",
        );
        assert_eq!(
            parse_body(body),
            [
                entry("4D5307E6_00000012", Some(0)),
                entry("GC4D5307E6.asset", Some(1_234_567)),
                entry("Content.db", Some(4_294_967_295)),
                entry("My Saved Game.bin", Some(24)),
            ]
        );
    }

    #[test]
    fn aurora_root_listing_shows_drives() {
        let body = concat!(
            "drwxrwxrwx   1 root root             0 Jan 01  2000 Game\r\n",
            "drwxrwxrwx   1 root root             0 Jan 01  2000 Hdd1\r\n",
            "drwxrwxrwx   1 root root             0 Jan 01  2000 Usb0\r\n",
        );
        assert_eq!(
            parse_body(body),
            [
                entry("Game", Some(0)),
                entry("Hdd1", Some(0)),
                entry("Usb0", Some(0)),
            ]
        );
    }

    #[test]
    fn aurora_empty_folder_lists_nothing() {
        assert_eq!(parse_body(""), []);
    }

    #[test]
    fn aurora_asset_is_found_whatever_case_is_asked() {
        let entries =
            parse_body("-rw-rw-rw-   1 root root         65536 Nov 03 14:05 GC4D5307E6.asset\r\n");
        assert_eq!(
            find_entry(&entries, "gc4d5307e6.ASSET"),
            Some(&entry("GC4D5307E6.asset", Some(65_536)))
        );
    }
}
