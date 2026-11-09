use super::release_platform::ReleasePlatform;
use crate::ui_review_zip::UI_SCREENSHOTS_ZIP;

pub const ASSET_PREFIX: &str = "AuroraAssetEditor";
pub const EXE_EXTENSION: &str = "exe";
pub const DMG_EXTENSION: &str = "dmg";
pub const APPIMAGE_EXTENSION: &str = "AppImage";
pub const TARBALL_EXTENSION: &str = "tar.gz";

/// `AuroraAssetEditor-<version>-<platform>`: every asset's name before its extension, and the
/// name of the folder inside a Linux tarball.
pub fn asset_stem(platform: ReleasePlatform, version: &str) -> String {
    format!("{ASSET_PREFIX}-{version}-{}", platform.name())
}

pub fn asset_name(platform: ReleasePlatform, version: &str, extension: &str) -> String {
    format!("{}.{extension}", asset_stem(platform, version))
}

/// The files `cargo xtask package` writes for `platform`, sorted.
pub fn package_assets(platform: ReleasePlatform, version: &str) -> Vec<String> {
    let extensions: &[&str] = match platform {
        ReleasePlatform::WindowsX86_64 => &[EXE_EXTENSION],
        ReleasePlatform::LinuxX86_64 | ReleasePlatform::LinuxAarch64 => {
            &[APPIMAGE_EXTENSION, TARBALL_EXTENSION]
        }
        ReleasePlatform::MacosUniversal => &[DMG_EXTENSION],
    };
    let mut names: Vec<String> = extensions
        .iter()
        .map(|extension| asset_name(platform, version, extension))
        .collect();
    names.sort();
    names
}

/// Every file a release of `version` carries besides its checksum list, sorted.
pub fn expected_release_assets(version: &str) -> Vec<String> {
    let mut names: Vec<String> = ReleasePlatform::ALL
        .into_iter()
        .flat_map(|platform| package_assets(platform, version))
        .collect();
    names.push(UI_SCREENSHOTS_ZIP.to_owned());
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_follow_the_pattern() {
        assert_eq!(
            package_assets(ReleasePlatform::LinuxAarch64, "2.0.7"),
            [
                "AuroraAssetEditor-2.0.7-linux-aarch64.AppImage",
                "AuroraAssetEditor-2.0.7-linux-aarch64.tar.gz"
            ]
        );
        assert_eq!(
            package_assets(ReleasePlatform::WindowsX86_64, "2.0.7"),
            ["AuroraAssetEditor-2.0.7-windows-x86_64.exe"]
        );
        assert_eq!(
            package_assets(ReleasePlatform::MacosUniversal, "2.0.7"),
            ["AuroraAssetEditor-2.0.7-macos-universal.dmg"]
        );
    }

    #[test]
    fn release_carries_every_package_and_the_screenshots() {
        assert_eq!(
            expected_release_assets("2.0.7"),
            [
                "AuroraAssetEditor-2.0.7-linux-aarch64.AppImage",
                "AuroraAssetEditor-2.0.7-linux-aarch64.tar.gz",
                "AuroraAssetEditor-2.0.7-linux-x86_64.AppImage",
                "AuroraAssetEditor-2.0.7-linux-x86_64.tar.gz",
                "AuroraAssetEditor-2.0.7-macos-universal.dmg",
                "AuroraAssetEditor-2.0.7-windows-x86_64.exe",
                "ui-screenshots.zip",
            ]
        );
    }
}
