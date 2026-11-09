# Aurora Asset Editor

[![Release](https://github.com/XboxUnity/AuroraAssetEditor/actions/workflows/release.yml/badge.svg?branch=rust-cross-platform)](https://github.com/XboxUnity/AuroraAssetEditor/actions/workflows/release.yml)

Aurora Asset Editor edits the game art that Aurora, the Xbox 360 dashboard, shows for each game: cover, background, icon, banner and screenshots. It opens and saves Aurora's `.asset` files, reads FreeStyle Dash (FSD) asset files, and works with the files on your console over FTP.

## Screenshots

| | Light | Dark |
|---|---|---|
| Cover | ![Cover page, light](docs/images/readme/cover-light.png) | ![Cover page, dark](docs/images/readme/cover-dark.png) |
| Screenshots | ![Screenshots page, light](docs/images/readme/screenshots-light.png) | ![Screenshots page, dark](docs/images/readme/screenshots-dark.png) |
| Console | ![Console page, light](docs/images/readme/console-light.png) | ![Console page, dark](docs/images/readme/console-dark.png) |
| Online | ![Online page, light](docs/images/readme/online-light.png) | ![Online page, dark](docs/images/readme/online-dark.png) |

## Download

Get the newest release from the [Releases page](https://github.com/XboxUnity/AuroraAssetEditor/releases).

| System | File | First run |
|---|---|---|
| Windows 10 and 11 (x64) | `AuroraAssetEditor-<version>-windows-x86_64.exe` | Windows may warn about an unknown publisher: choose More info, then Run anyway. |
| macOS 11 or later (Apple silicon and Intel) | `AuroraAssetEditor-<version>-macos-universal.dmg` | The app is not signed by Apple. Follow `First launch.txt` in the disk image. |
| Linux (x64 or arm64) | `AuroraAssetEditor-<version>-linux-<arch>.AppImage`, or the `.tar.gz` with the plain binary | Make the file executable (`chmod +x`). Needs glibc 2.35 or later (Ubuntu 22.04, Debian 12 or newer) and the usual desktop libraries. Dragging files into the window does not work under Wayland; use Open. |

## What it does

- Edit the cover, background, icon, banner and up to 20 screenshots of a game, from image files (PNG, JPEG, BMP, GIF, TIFF, WebP, TGA, ICO, QOI, DDS) or dropped onto the window.
- Save and open Aurora `.asset` files, one per asset type, exactly as Aurora expects them, compressed or not.
- Connect to Aurora's FTP server: list the console's games, open a game's assets, upload, download or remove them.
- Find covers on XboxUnity and the Internet Archive, and fetch covers for all the console's games in one go, skipping those that already have one unless you choose to replace them.

## Building from source

Install [Rust](https://rustup.rs); the repository pins its toolchain in `rust-toolchain.toml`. On Ubuntu or Debian, install `build-essential`, `pkg-config`, `libwayland-dev`, `libfontconfig1-dev` and `libxkbcommon-dev` first.

    cargo build --release -p aurora-asset-editor
    cargo test --workspace

The checks against the original `AuroraAsset.dll` (`cargo xtask golden check`, `cargo xtask cross-decode`) run on Windows only.

## Credits

A huge thanks goes out to MaesterRowen for explaining the format, and for the AuroraAsset.dll library that handled the image conversion (between D3DTexture and Bitmap) in 1.x. Version 2 does the conversion itself and is tested against his library.
Many thanks also goes to Mattie for the placeholder images, still used in version 2 :)

Thanks also goes to gavin_darkglider & felida for helping me test this thing :)

Thanks to RAP7HOR for the icon used in the application as of version 1.1!

Third-party code and fonts are listed with their licences in the app, under About.

## Licence

Public domain, see [UNLICENSE](UNLICENSE).
