//! Calls into the original 32-bit `AuroraAsset.dll`.
//!
//! Every export uses the same two-pass protocol: call once with null output buffers to learn
//! the sizes, allocate, then call again to fill them.

#![allow(unsafe_code)]

use libloading::os::windows::{LOAD_WITH_ALTERED_SEARCH_PATH, Library};
use std::ffi::c_int;
use std::path::Path;
use std::ptr::null_mut;

const DLL_SUCCESS: c_int = 1;
const BGRA_BYTES_PER_PIXEL: usize = 4;

type ImageToAssetFn = unsafe extern "C" fn(
    image: *const u8,
    image_len: c_int,
    width: c_int,
    height: c_int,
    use_compression: c_int,
    header: *mut u8,
    header_len: *mut c_int,
    video: *mut u8,
    video_len: *mut c_int,
) -> c_int;

type AssetToImageFn = unsafe extern "C" fn(
    header: *mut u8,
    header_len: c_int,
    video: *const u8,
    video_len: c_int,
    image: *mut u8,
    image_len: *mut c_int,
    width: *mut c_int,
    height: *mut c_int,
) -> c_int;

type DdsToImageFn = unsafe extern "C" fn(
    dds: *const u8,
    dds_len: c_int,
    image: *mut u8,
    image_len: *mut c_int,
    width: *mut c_int,
    height: *mut c_int,
) -> c_int;

pub struct AuroraAssetDll {
    image_to_asset: ImageToAssetFn,
    asset_to_image: AssetToImageFn,
    dds_to_image: DdsToImageFn,
    // Keeps the DLL mapped for as long as the function pointers above are in use.
    _library: Library,
}

impl AuroraAssetDll {
    pub fn load(path: &Path) -> Result<Self, libloading::Error> {
        // The altered search path lets msvcr100.dll resolve from the DLL's own folder.
        // SAFETY: loading the reference DLL runs its initialisation code, which is the point.
        let library = unsafe { Library::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH)? };
        // SAFETY: the symbol types match the DLL's documented cdecl exports.
        unsafe {
            let image_to_asset = *library.get::<ImageToAssetFn>(b"ConvertImageToAsset\0")?;
            let asset_to_image = *library.get::<AssetToImageFn>(b"ConvertAssetToImage\0")?;
            let dds_to_image = *library.get::<DdsToImageFn>(b"ConvertDDSToImage\0")?;
            Ok(Self {
                image_to_asset,
                asset_to_image,
                dds_to_image,
                _library: library,
            })
        }
    }

    /// Returns `(header, video)`, or `None` if the DLL refuses the input.
    pub fn image_to_asset(
        &self,
        argb: &[u8],
        width: u32,
        height: u32,
        compress: bool,
    ) -> Option<(Vec<u8>, Vec<u8>)> {
        let image_len = argb.len() as c_int;
        let mut header_len: c_int = 0;
        let mut video_len: c_int = 0;
        // SAFETY: null output buffers request sizes only; the input is valid for `image_len`.
        let sized = unsafe {
            (self.image_to_asset)(
                argb.as_ptr(),
                image_len,
                width as c_int,
                height as c_int,
                compress as c_int,
                null_mut(),
                &mut header_len,
                null_mut(),
                &mut video_len,
            )
        };
        if sized != DLL_SUCCESS {
            return None;
        }

        let mut header = vec![0u8; header_len as usize];
        let mut video = vec![0u8; video_len as usize];
        // SAFETY: both buffers are allocated to the sizes the DLL reported.
        let filled = unsafe {
            (self.image_to_asset)(
                argb.as_ptr(),
                image_len,
                width as c_int,
                height as c_int,
                compress as c_int,
                header.as_mut_ptr(),
                &mut header_len,
                video.as_mut_ptr(),
                &mut video_len,
            )
        };
        if filled != DLL_SUCCESS {
            return None;
        }
        header.truncate(header_len as usize);
        video.truncate(video_len as usize);
        Some((header, video))
    }

    /// Returns `(bgra, width, height)`, or `None` if the DLL refuses the input.
    pub fn asset_to_image(&self, header: &[u8], video: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
        // The DLL byte-swaps the header in place, so it gets a private copy.
        let mut header_copy = header.to_vec();
        let header_len = header.len() as c_int;
        let video_len = video.len() as c_int;
        let mut image_len: c_int = 0;
        let mut width: c_int = 0;
        let mut height: c_int = 0;
        // SAFETY: null image buffer requests the size only; inputs are valid for their lengths.
        let sized = unsafe {
            (self.asset_to_image)(
                header_copy.as_mut_ptr(),
                header_len,
                video.as_ptr(),
                video_len,
                null_mut(),
                &mut image_len,
                &mut width,
                &mut height,
            )
        };
        if sized != DLL_SUCCESS {
            return None;
        }

        let mut image = vec![0u8; image_len as usize];
        // SAFETY: the image buffer is allocated to the size the DLL reported.
        let filled = unsafe {
            (self.asset_to_image)(
                header_copy.as_mut_ptr(),
                header_len,
                video.as_ptr(),
                video_len,
                image.as_mut_ptr(),
                &mut image_len,
                &mut width,
                &mut height,
            )
        };
        if filled != DLL_SUCCESS {
            return None;
        }
        image.truncate(image_len as usize);
        Some((image, width as u32, height as u32))
    }

    /// Returns `(buffer, width, height, reported_len)`, or `None` if the DLL refuses the input.
    /// The buffer holds the full `width * height * 4` bytes the DLL writes.
    pub fn dds_to_image(&self, dds: &[u8]) -> Option<(Vec<u8>, u32, u32, u32)> {
        let dds_len = dds.len() as c_int;
        let mut image_len: c_int = 0;
        let mut width: c_int = 0;
        let mut height: c_int = 0;
        // SAFETY: null image buffer requests the size only; the input is valid for `dds_len`.
        let sized = unsafe {
            (self.dds_to_image)(
                dds.as_ptr(),
                dds_len,
                null_mut(),
                &mut image_len,
                &mut width,
                &mut height,
            )
        };
        if sized != DLL_SUCCESS {
            return None;
        }

        // The DLL reports the header's pitch or linear size as the image length, yet writes the full
        // `width * height * 4` bytes, so the buffer is sized for what it writes.
        let written_len = (width as usize) * (height as usize) * BGRA_BYTES_PER_PIXEL;
        let mut image = vec![0u8; written_len.max(image_len as usize)];
        // SAFETY: the buffer holds the larger of the reported length and the full decoded image.
        let filled = unsafe {
            (self.dds_to_image)(
                dds.as_ptr(),
                dds_len,
                image.as_mut_ptr(),
                &mut image_len,
                &mut width,
                &mut height,
            )
        };
        if filled != DLL_SUCCESS {
            return None;
        }
        image.truncate(written_len);
        Some((image, width as u32, height as u32, image_len as u32))
    }
}
