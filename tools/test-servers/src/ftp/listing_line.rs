use super::ListingStyle;
use super::file_system::FolderEntry;

const ROOT_DATE: &str = "Jan 01  2000";

/// One listing line in the given style, reporting `size` for a file. `at_root` selects Aurora's
/// drive lines. Every date derives from the entry, so the output is deterministic.
pub(crate) fn listing_line(
    style: ListingStyle,
    entry: &FolderEntry,
    size: u64,
    at_root: bool,
) -> String {
    let name = &entry.name;
    match (style, entry.is_folder) {
        (ListingStyle::Aurora, _) if at_root => {
            format!("drwxrwxrwx   1 root root {:>13} {ROOT_DATE} {name}", 0)
        }
        (ListingStyle::Aurora, false) => {
            let date = entry.modified.aurora_text();
            format!("-rw-rw-rw-   1 root root {size:>13} {date} {name}")
        }
        (ListingStyle::Aurora, true) => {
            let date = entry.modified.aurora_text();
            format!("drwxrwxrwx   1 root root {:>13} {date} {name}", 0)
        }
        (ListingStyle::Unix, false) => {
            format!("-rw-r--r--   1 root  root  {size:>10} Nov 07 12:00 {name}")
        }
        (ListingStyle::Unix, true) => {
            format!("drwxr-xr-x   1 root  root  {:>10} Nov 07 12:00 {name}", 0)
        }
        (ListingStyle::Dos, false) => format!("11-07-26  12:00PM {size:>20} {name}"),
        (ListingStyle::Dos, true) => format!("11-07-26  12:00PM       <DIR>          {name}"),
        (ListingStyle::Mlsd, false) => {
            format!("type=file;size={size};modify=20261107120000; {name}")
        }
        (ListingStyle::Mlsd, true) => format!("type=dir;modify=20261107120000; {name}"),
    }
}
