use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use aurora_asset::RgbaImage;

use super::app_icon::{load_app_icon, resized_icon, write_png};
use super::asset_names::{DMG_EXTENSION, asset_name};
use super::release_platform::ReleasePlatform;
use super::staging::{
    copy_file, copy_folder, create_folder, make_executable, recreate_folder, symlink, write_text,
};
use super::tool_command::{ToolCommand, path_argument};

/// The universal binary inside the bundle, from the repository root. The workflow smoke-tests
/// this path as written.
pub const MACOS_BUNDLE_BINARY: &str =
    "target/package/macos-universal/Aurora Asset Editor.app/Contents/MacOS/aurora-asset-editor";

const APP_NAME: &str = "Aurora Asset Editor";
const BUNDLE_FOLDER: &str = "Aurora Asset Editor.app";
/// Never changes once released: macOS keys the app's settings and permissions by it.
const BUNDLE_IDENTIFIER: &str = "net.xboxunity.AuroraAssetEditor";
const BUNDLE_EXECUTABLE: &str = "aurora-asset-editor";
const ICON_NAME: &str = "AppIcon";
const ICON_FILE: &str = "AppIcon.icns";
const ICONSET_FOLDER: &str = "AppIcon.iconset";
/// The first macOS on Apple silicon, so one value covers both slices.
const MINIMUM_SYSTEM_VERSION: &str = "11.0";
const COPYRIGHT: &str = "Public domain (Unlicense)";

const DMG_STAGING_FOLDER: &str = "dmg";
const FIRST_LAUNCH_SOURCE: &str = "packaging/macos-first-launch.txt";
const FIRST_LAUNCH_NAME: &str = "First launch.txt";
const APPLICATIONS_LINK: &str = "Applications";
const APPLICATIONS_FOLDER: &str = "/Applications";

/// `hdiutil create` fails now and then on hosted runners with "Resource busy"
/// (actions/runner-images#7522); only that failure is retried.
const HDIUTIL_ATTEMPTS: u32 = 3;
const HDIUTIL_BUSY: &str = "Resource busy";
const HDIUTIL_RETRY_DELAY: Duration = Duration::from_secs(5);

/// The images of an `.iconset` folder, with their side in pixels.
const ICONSET_IMAGES: [(&str, u32); 10] = [
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024),
];

/// What `package` hands the macOS packaging.
pub struct MacosInputs<'a> {
    pub version: &'a str,
    pub build_label: &'a str,
    pub arm64_binary: &'a Path,
    pub x86_64_binary: &'a Path,
    pub work_folder: &'a Path,
    pub out: &'a Path,
}

/// Where the macOS packaging reads, stages and writes.
struct MacosPaths {
    arm64_binary: PathBuf,
    x86_64_binary: PathBuf,
    app: PathBuf,
    universal_binary: PathBuf,
    iconset: PathBuf,
    dmg_staging: PathBuf,
    dmg: PathBuf,
    volume_name: String,
}

impl MacosPaths {
    fn new(root: &Path, inputs: &MacosInputs) -> MacosPaths {
        let app = inputs.work_folder.join(BUNDLE_FOLDER);
        MacosPaths {
            arm64_binary: inputs.arm64_binary.to_path_buf(),
            x86_64_binary: inputs.x86_64_binary.to_path_buf(),
            universal_binary: root.join(MACOS_BUNDLE_BINARY),
            iconset: inputs.work_folder.join(ICONSET_FOLDER),
            dmg_staging: inputs.work_folder.join(DMG_STAGING_FOLDER),
            dmg: inputs.out.join(asset_name(
                ReleasePlatform::MacosUniversal,
                inputs.version,
                DMG_EXTENSION,
            )),
            volume_name: format!("{APP_NAME} {}", inputs.version),
            app,
        }
    }

    fn contents(&self) -> PathBuf {
        self.app.join("Contents")
    }

    fn universal_binary_folder(&self) -> PathBuf {
        self.contents().join("MacOS")
    }

    fn resources(&self) -> PathBuf {
        self.contents().join("Resources")
    }
}

/// The tools the macOS packaging runs, in order: the universal binary, the icon, the ad-hoc
/// signature and its check, the disk image.
fn macos_commands(paths: &MacosPaths) -> Vec<ToolCommand> {
    let app = path_argument(&paths.app);
    vec![
        ToolCommand::new(
            "lipo",
            [
                "-create".to_owned(),
                path_argument(&paths.arm64_binary),
                path_argument(&paths.x86_64_binary),
                "-output".to_owned(),
                path_argument(&paths.universal_binary),
            ],
        ),
        ToolCommand::new(
            "iconutil",
            [
                "-c".to_owned(),
                "icns".to_owned(),
                "-o".to_owned(),
                path_argument(&paths.resources().join(ICON_FILE)),
                path_argument(&paths.iconset),
            ],
        ),
        // Without a signature Apple silicon reports the app as damaged; ad-hoc is enough to
        // reach the first-launch prompt.
        ToolCommand::new("codesign", ["--force", "--deep", "--sign", "-", &app]),
        ToolCommand::new("codesign", ["--verify", "--deep", "--strict", &app]),
        ToolCommand::new(
            "hdiutil",
            [
                "create".to_owned(),
                "-volname".to_owned(),
                paths.volume_name.clone(),
                "-srcfolder".to_owned(),
                path_argument(&paths.dmg_staging),
                "-format".to_owned(),
                "UDZO".to_owned(),
                "-ov".to_owned(),
                path_argument(&paths.dmg),
            ],
        ),
    ]
}

/// Builds the signed universal bundle in the work folder and writes the disk image into the
/// output folder. Returns the written file.
pub fn package_macos(root: &Path, inputs: &MacosInputs) -> Result<Vec<PathBuf>> {
    let paths = MacosPaths::new(root, inputs);
    let [
        create_universal_binary,
        create_icon,
        sign,
        verify_signature,
        create_dmg,
    ] = <[ToolCommand; 5]>::try_from(macos_commands(&paths))
        .expect("macos_commands builds five commands");

    create_folder(&paths.universal_binary_folder())?;
    create_folder(&paths.resources())?;
    write_text(
        &paths.contents().join("Info.plist"),
        &info_plist(inputs.version, inputs.build_label),
    )?;
    create_universal_binary.run()?;
    make_executable(&paths.universal_binary)?;

    write_iconset(&load_app_icon(root)?, &paths.iconset)?;
    create_icon.run()?;

    sign.run()?;
    verify_signature.run()?;

    stage_dmg(root, &paths)?;
    run_hdiutil(&create_dmg)?;
    Ok(vec![paths.dmg])
}

/// The disk image's contents: the signed bundle, a link to Applications to drag it onto, and
/// the first-launch note.
fn stage_dmg(root: &Path, paths: &MacosPaths) -> Result<()> {
    recreate_folder(&paths.dmg_staging)?;
    copy_folder(&paths.app, &paths.dmg_staging.join(BUNDLE_FOLDER))?;
    symlink(
        APPLICATIONS_FOLDER,
        &paths.dmg_staging.join(APPLICATIONS_LINK),
    )?;
    copy_file(
        &root.join(FIRST_LAUNCH_SOURCE),
        &paths.dmg_staging.join(FIRST_LAUNCH_NAME),
    )
}

fn run_hdiutil(create_dmg: &ToolCommand) -> Result<()> {
    let mut attempt = 1;
    loop {
        let output = create_dmg.output()?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !should_retry_hdiutil(&stderr, attempt) {
            return Err(create_dmg.failure(&output));
        }
        eprintln!(
            "hdiutil create failed with \"{HDIUTIL_BUSY}\" (attempt {attempt} of {HDIUTIL_ATTEMPTS}); trying again in {} s",
            HDIUTIL_RETRY_DELAY.as_secs()
        );
        thread::sleep(HDIUTIL_RETRY_DELAY);
        attempt += 1;
    }
}

fn should_retry_hdiutil(stderr: &str, attempt: u32) -> bool {
    attempt < HDIUTIL_ATTEMPTS && stderr.contains(HDIUTIL_BUSY)
}

/// The iconset sizes that do not need the icon enlarged.
fn iconset_images(icon_side: u32) -> Vec<(&'static str, u32)> {
    ICONSET_IMAGES
        .into_iter()
        .filter(|&(_, side)| side <= icon_side)
        .collect()
}

fn write_iconset(icon: &RgbaImage, folder: &Path) -> Result<()> {
    create_folder(folder)?;
    let icon_side = icon.width().min(icon.height());
    for (name, side) in iconset_images(icon_side) {
        write_png(&resized_icon(icon, side), &folder.join(name))?;
    }
    Ok(())
}

pub fn info_plist(version: &str, build_label: &str) -> String {
    let entries = [
        ("CFBundleName", APP_NAME),
        ("CFBundleDisplayName", APP_NAME),
        ("CFBundleIdentifier", BUNDLE_IDENTIFIER),
        ("CFBundleExecutable", BUNDLE_EXECUTABLE),
        ("CFBundleIconFile", ICON_NAME),
        ("CFBundlePackageType", "APPL"),
        ("CFBundleShortVersionString", version),
        ("CFBundleVersion", version),
        ("CFBundleGetInfoString", build_label),
        ("LSMinimumSystemVersion", MINIMUM_SYSTEM_VERSION),
        ("NSHumanReadableCopyright", COPYRIGHT),
    ];
    let mut plist = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
        "<plist version=\"1.0\">\n",
        "<dict>\n",
    ));
    for (key, value) in entries {
        plist.push_str(&format!(
            "\t<key>{key}</key>\n\t<string>{}</string>\n",
            xml_escaped(value)
        ));
    }
    plist.push_str("\t<key>NSHighResolutionCapable</key>\n\t<true/>\n");
    plist.push_str("</dict>\n</plist>\n");
    plist
}

fn xml_escaped(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notices::repository_root;

    fn sample_paths() -> MacosPaths {
        let root = Path::new("/repo");
        let inputs = MacosInputs {
            version: "2.0.7",
            build_label: "2.0.7 (a1b2c3d)",
            arm64_binary: Path::new(
                "/repo/target/aarch64-apple-darwin/release/aurora-asset-editor",
            ),
            x86_64_binary: Path::new(
                "/repo/target/x86_64-apple-darwin/release/aurora-asset-editor",
            ),
            work_folder: &crate::package::work_folder(root, ReleasePlatform::MacosUniversal),
            out: Path::new("/repo/dist"),
        };
        MacosPaths::new(root, &inputs)
    }

    /// The `<string>` right after `<key>{key}</key>`.
    fn plist_value<'a>(plist: &'a str, key: &str) -> &'a str {
        let key_line = format!("<key>{key}</key>\n\t<string>");
        let start = plist.find(&key_line).expect("the key is in the plist") + key_line.len();
        let length = plist[start..]
            .find("</string>")
            .expect("the value is closed");
        &plist[start..start + length]
    }

    #[test]
    fn info_plist_carries_the_version_and_build_label() {
        let plist = info_plist("2.0.7", "2.0.7 (a1b2c3d)");

        assert_eq!(plist_value(&plist, "CFBundleShortVersionString"), "2.0.7");
        assert_eq!(plist_value(&plist, "CFBundleVersion"), "2.0.7");
        assert_eq!(
            plist_value(&plist, "CFBundleGetInfoString"),
            "2.0.7 (a1b2c3d)"
        );
        assert_eq!(
            plist_value(&plist, "CFBundleIdentifier"),
            "net.xboxunity.AuroraAssetEditor"
        );
        assert_eq!(plist_value(&plist, "LSMinimumSystemVersion"), "11.0");
        assert_eq!(
            plist_value(&plist, "CFBundleExecutable"),
            "aurora-asset-editor"
        );
        assert!(plist.contains("<key>NSHighResolutionCapable</key>\n\t<true/>"));

        let escaped = info_plist("2.0.7", "a<b");
        assert_eq!(plist_value(&escaped, "CFBundleGetInfoString"), "a&lt;b");
    }

    #[test]
    fn macos_commands_sign_and_verify_the_bundle() {
        let paths = sample_paths();
        let app = path_argument(&paths.app);

        let commands = macos_commands(&paths);

        let programs: Vec<&str> = commands
            .iter()
            .map(|command| command.program.as_str())
            .collect();
        assert_eq!(
            programs,
            ["lipo", "iconutil", "codesign", "codesign", "hdiutil"]
        );
        assert_eq!(
            commands[2].arguments,
            ["--force", "--deep", "--sign", "-", app.as_str()]
        );
        assert_eq!(
            commands[3].arguments,
            ["--verify", "--deep", "--strict", app.as_str()]
        );
        assert_eq!(
            commands[0].arguments[..3],
            [
                "-create",
                "/repo/target/aarch64-apple-darwin/release/aurora-asset-editor",
                "/repo/target/x86_64-apple-darwin/release/aurora-asset-editor",
            ]
        );
        assert_eq!(
            commands[4].arguments,
            [
                "create".to_owned(),
                "-volname".to_owned(),
                "Aurora Asset Editor 2.0.7".to_owned(),
                "-srcfolder".to_owned(),
                path_argument(&paths.dmg_staging),
                "-format".to_owned(),
                "UDZO".to_owned(),
                "-ov".to_owned(),
                path_argument(
                    &Path::new("/repo/dist").join("AuroraAssetEditor-2.0.7-macos-universal.dmg")
                ),
            ]
        );
    }

    #[test]
    fn bundle_binary_is_where_lipo_writes_it() {
        let paths = sample_paths();

        assert_eq!(
            paths.universal_binary,
            paths.universal_binary_folder().join(BUNDLE_EXECUTABLE)
        );
        assert!(paths.universal_binary.starts_with(&paths.app));
        assert_eq!(
            macos_commands(&paths)[0].arguments[4],
            path_argument(&paths.universal_binary)
        );
    }

    #[test]
    fn hdiutil_is_retried_only_while_the_resource_is_busy() {
        let busy = "hdiutil: create failed - Resource busy";

        assert!(should_retry_hdiutil(busy, 1));
        assert!(should_retry_hdiutil(busy, HDIUTIL_ATTEMPTS - 1));
        assert!(!should_retry_hdiutil(busy, HDIUTIL_ATTEMPTS));
        assert!(!should_retry_hdiutil(
            "hdiutil: create failed - No space left on device",
            1
        ));
    }

    #[test]
    fn iconset_holds_the_sizes_the_app_icon_covers() {
        let folder = tempfile::tempdir().unwrap();
        let icon = load_app_icon(&repository_root()).unwrap();

        write_iconset(&icon, folder.path()).unwrap();

        let mut written: Vec<(String, u32)> = std::fs::read_dir(folder.path())
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let image = image::open(&path).unwrap();
                (
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    image.width(),
                )
            })
            .collect();
        written.sort();
        assert_eq!(
            written,
            [
                ("icon_16x16.png".to_owned(), 16),
                ("icon_16x16@2x.png".to_owned(), 32),
                ("icon_32x32.png".to_owned(), 32),
            ]
        );
    }
}
