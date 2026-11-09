# macOS iconset

`cargo xtask package macos-universal` copies these images into `AppIcon.iconset` and turns them into the bundle's `AppIcon.icns` with `iconutil`. The file names are the ones `iconutil` expects, and each image has the size its name says.

They were made once from the largest image in `crates/aurora-ui/ui/app_icon.ico` (48x48, 32-bit), scaled with the `image` crate's Lanczos3 filter (`imageops::resize`): down to 16 and 32 pixels, and up to 64, 128, 256, 512 and 1024. The enlarged sizes are soft, because the app has no larger icon.

When a real large icon exists (at least 1024x1024), replace every image here with one made from it, keeping the names and sizes. The packaging checks each size.
