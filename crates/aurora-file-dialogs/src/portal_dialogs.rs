use std::ffi::OsString;
use std::io::{self, Write};
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use ashpd::WindowIdentifier;
use ashpd::desktop::ResponseError;
use ashpd::desktop::file_chooser::{FileFilter, SelectedFiles};

use super::file_uri::file_uri_path_bytes;
use super::{DialogError, DialogFilter, DialogParent, DialogRequest};

pub fn show_startup_error(title: &str, text: &str) -> io::Result<()> {
    writeln!(io::stderr(), "{title}: {text}")
}

pub fn open_files(request: &DialogRequest) -> Result<Vec<PathBuf>, DialogError> {
    async_io::block_on(open(request, true)).map(|picked| picked.unwrap_or_default())
}

pub fn open_file(request: &DialogRequest) -> Result<Option<PathBuf>, DialogError> {
    let picked = async_io::block_on(open(request, false))?;
    Ok(picked.and_then(|paths| paths.into_iter().next()))
}

pub fn save_file(request: &DialogRequest) -> Result<Option<PathBuf>, DialogError> {
    let picked = async_io::block_on(async {
        let (filters, current_filter) = portal_filters(request);
        let sent = SelectedFiles::save_file()
            .identifier(window_identifier(request.parent).await)
            .title(request.title.as_str())
            .modal(true)
            .current_name(request.default_name.as_deref())
            .filters(filters)
            .current_filter(current_filter)
            .send()
            .await;
        picked_paths(sent.and_then(|answer| answer.response()))
    })?;
    Ok(picked.and_then(|paths| paths.into_iter().next()))
}

pub fn pick_folder(
    title: &str,
    parent: Option<DialogParent>,
) -> Result<Option<PathBuf>, DialogError> {
    let picked = async_io::block_on(async {
        let sent = SelectedFiles::open_file()
            .identifier(window_identifier(parent).await)
            .title(title)
            .modal(true)
            .directory(true)
            .send()
            .await;
        picked_paths(sent.and_then(|answer| answer.response()))
    })?;
    Ok(picked.and_then(|paths| paths.into_iter().next()))
}

/// `None` when the user cancelled.
async fn open(
    request: &DialogRequest<'_>,
    multiple: bool,
) -> Result<Option<Vec<PathBuf>>, DialogError> {
    let (filters, current_filter) = portal_filters(request);
    let sent = SelectedFiles::open_file()
        .identifier(window_identifier(request.parent).await)
        .title(request.title.as_str())
        .modal(true)
        .multiple(multiple)
        .filters(filters)
        .current_filter(current_filter)
        .send()
        .await;
    picked_paths(sent.and_then(|answer| answer.response()))
}

fn picked_paths(
    answer: Result<SelectedFiles, ashpd::Error>,
) -> Result<Option<Vec<PathBuf>>, DialogError> {
    match answer {
        Ok(selected) => selected
            .uris()
            .iter()
            .map(|uri| file_uri_path(uri.as_str()))
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Err(ashpd::Error::Response(ResponseError::Cancelled)) => Ok(None),
        Err(error) => Err(DialogError::new(error)),
    }
}

fn portal_filters(request: &DialogRequest) -> (Vec<FileFilter>, Option<FileFilter>) {
    let filters: Vec<FileFilter> = request.filters.iter().map(portal_filter).collect();
    let current_filter = request
        .filters
        .get(request.default_filter)
        .map(portal_filter);
    (filters, current_filter)
}

fn portal_filter(filter: &DialogFilter) -> FileFilter {
    filter
        .patterns
        .iter()
        .fold(FileFilter::new(&filter.name), |portal, pattern| {
            portal.glob(pattern)
        })
}

/// The portal finds the parent window by its X11 id or Wayland surface; without one the dialog opens unparented.
async fn window_identifier(parent: Option<DialogParent<'_>>) -> Option<WindowIdentifier> {
    let parent = parent?;
    WindowIdentifier::from_raw_handle(&parent.window.as_raw(), Some(&parent.display.as_raw())).await
}

/// The portal answers with `file://` URIs, percent-encoded.
fn file_uri_path(uri: &str) -> Result<PathBuf, DialogError> {
    let bytes = file_uri_path_bytes(uri).ok_or_else(|| {
        DialogError::new(format!(
            "the file dialog returned {uri}, which is not a local file"
        ))
    })?;
    Ok(PathBuf::from(OsString::from_vec(bytes)))
}
