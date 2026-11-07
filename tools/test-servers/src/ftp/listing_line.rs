use super::ListingStyle;
use super::file_system::FolderEntry;

/// One listing line in the given style, reporting `size` for a file. The date is fixed, so the
/// output is deterministic.
pub(crate) fn listing_line(style: ListingStyle, entry: &FolderEntry, size: u64) -> String {
    let name = &entry.name;
    match (style, entry.is_folder) {
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
