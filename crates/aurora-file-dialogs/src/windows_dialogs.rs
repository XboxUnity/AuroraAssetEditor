use std::ffi::{OsString, c_void};
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use raw_window_handle::RawWindowHandle;
use windows::Win32::Foundation::{ERROR_CANCELLED, HWND, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FILEOPENDIALOGOPTIONS, FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
    FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, FileSaveDialog,
    IFileDialog, IFileOpenDialog, IFileSaveDialog, IShellItem, SIGDN_FILESYSPATH,
};
use windows::core::{GUID, HSTRING, Interface, PCWSTR};

use super::{DialogError, DialogParent, DialogRequest};

const PATTERN_SEPARATOR: &str = ";";
const EXTENSION_PATTERN_PREFIX: &str = "*.";

pub fn open_files(request: &DialogRequest) -> Result<Vec<PathBuf>, DialogError> {
    let _com = ComApartment::enter()?;
    let dialog: IFileOpenDialog = create(&FileOpenDialog)?;
    prepare(&dialog, request, FOS_ALLOWMULTISELECT | FOS_FILEMUSTEXIST)?;
    if !show(&dialog, request.parent)? {
        return Ok(Vec::new());
    }
    let items = unsafe { dialog.GetResults() }.map_err(DialogError::new)?;
    let count = unsafe { items.GetCount() }.map_err(DialogError::new)?;
    (0..count)
        .map(|index| {
            let item = unsafe { items.GetItemAt(index) }.map_err(DialogError::new)?;
            file_path(&item)
        })
        .collect()
}

pub fn open_file(request: &DialogRequest) -> Result<Option<PathBuf>, DialogError> {
    let _com = ComApartment::enter()?;
    let dialog: IFileOpenDialog = create(&FileOpenDialog)?;
    prepare(&dialog, request, FOS_FILEMUSTEXIST)?;
    picked_item(&dialog, request.parent)
}

pub fn save_file(request: &DialogRequest) -> Result<Option<PathBuf>, DialogError> {
    let _com = ComApartment::enter()?;
    let dialog: IFileSaveDialog = create(&FileSaveDialog)?;
    prepare(&dialog, request, FOS_OVERWRITEPROMPT)?;
    // Like the old SaveFileDialog's AddExtension: a name typed without an extension gets the chosen filter's.
    if let Some(extension) = default_extension(request) {
        unsafe { dialog.SetDefaultExtension(&HSTRING::from(extension)) }
            .map_err(DialogError::new)?;
    }
    picked_item(&dialog, request.parent)
}

pub fn pick_folder(
    title: &str,
    parent: Option<DialogParent>,
) -> Result<Option<PathBuf>, DialogError> {
    let _com = ComApartment::enter()?;
    let dialog: IFileOpenDialog = create(&FileOpenDialog)?;
    unsafe { dialog.SetTitle(&HSTRING::from(title)) }.map_err(DialogError::new)?;
    add_options(&dialog, FOS_PICKFOLDERS)?;
    picked_item(&dialog, parent)
}

fn create<T: Interface>(class: &GUID) -> Result<T, DialogError> {
    unsafe { CoCreateInstance(class, None, CLSCTX_INPROC_SERVER) }.map_err(DialogError::new)
}

fn prepare(
    dialog: &IFileDialog,
    request: &DialogRequest,
    options: FILEOPENDIALOGOPTIONS,
) -> Result<(), DialogError> {
    unsafe { dialog.SetTitle(&HSTRING::from(request.title.as_str())) }.map_err(DialogError::new)?;
    set_file_types(dialog, request)?;
    if let Some(name) = &request.default_name {
        unsafe { dialog.SetFileName(&HSTRING::from(name.as_str())) }.map_err(DialogError::new)?;
    }
    add_options(dialog, options)
}

fn set_file_types(dialog: &IFileDialog, request: &DialogRequest) -> Result<(), DialogError> {
    if request.filters.is_empty() {
        return Ok(());
    }
    let texts: Vec<(HSTRING, HSTRING)> = request
        .filters
        .iter()
        .map(|filter| {
            (
                HSTRING::from(filter.name.as_str()),
                HSTRING::from(filter.patterns.join(PATTERN_SEPARATOR)),
            )
        })
        .collect();
    let specs: Vec<COMDLG_FILTERSPEC> = texts
        .iter()
        .map(|(name, patterns)| COMDLG_FILTERSPEC {
            pszName: PCWSTR(name.as_ptr()),
            pszSpec: PCWSTR(patterns.as_ptr()),
        })
        .collect();
    unsafe { dialog.SetFileTypes(&specs) }.map_err(DialogError::new)?;
    // The dialog counts file types from 1.
    let index = u32::try_from(request.default_filter + 1).map_err(DialogError::new)?;
    unsafe { dialog.SetFileTypeIndex(index) }.map_err(DialogError::new)
}

fn add_options(dialog: &IFileDialog, options: FILEOPENDIALOGOPTIONS) -> Result<(), DialogError> {
    let current = unsafe { dialog.GetOptions() }.map_err(DialogError::new)?;
    let wanted = current | options | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST;
    unsafe { dialog.SetOptions(wanted) }.map_err(DialogError::new)
}

/// `false` when the user cancelled.
fn show(dialog: &IFileDialog, parent: Option<DialogParent>) -> Result<bool, DialogError> {
    match unsafe { dialog.Show(owner_window(parent)) } {
        Ok(()) => Ok(true),
        Err(error) if error.code() == ERROR_CANCELLED.to_hresult() => Ok(false),
        Err(error) => Err(DialogError::new(error)),
    }
}

fn picked_item(
    dialog: &IFileDialog,
    parent: Option<DialogParent>,
) -> Result<Option<PathBuf>, DialogError> {
    if !show(dialog, parent)? {
        return Ok(None);
    }
    let item = unsafe { dialog.GetResult() }.map_err(DialogError::new)?;
    file_path(&item).map(Some)
}

fn file_path(item: &IShellItem) -> Result<PathBuf, DialogError> {
    let name = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }.map_err(DialogError::new)?;
    let path = PathBuf::from(OsString::from_wide(unsafe { name.as_wide() }));
    unsafe { CoTaskMemFree(Some(name.0 as *const c_void)) };
    Ok(path)
}

fn owner_window(parent: Option<DialogParent>) -> Option<HWND> {
    match parent.map(|parent| parent.window.as_raw()) {
        Some(RawWindowHandle::Win32(handle)) => Some(HWND(handle.hwnd.get() as *mut c_void)),
        _ => None,
    }
}

fn default_extension<'a>(request: &'a DialogRequest<'_>) -> Option<&'a str> {
    request
        .filters
        .get(request.default_filter)?
        .patterns
        .first()?
        .strip_prefix(EXTENSION_PATTERN_PREFIX)
}

/// COM entered on this thread for the time of one dialog. The UI thread usually is in COM already (winit
/// initialises OLE for drag and drop), which `CoInitializeEx` also reports as success; that is undone too.
struct ComApartment {
    entered_here: bool,
}

impl ComApartment {
    fn enter() -> Result<ComApartment, DialogError> {
        let result =
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        if result == RPC_E_CHANGED_MODE {
            // The thread is in another apartment already; the dialogs work there too, and it is not ours to end.
            return Ok(ComApartment {
                entered_here: false,
            });
        }
        result.ok().map_err(DialogError::new)?;
        Ok(ComApartment { entered_here: true })
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.entered_here {
            unsafe { CoUninitialize() };
        }
    }
}
