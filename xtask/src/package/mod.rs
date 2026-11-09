//! `cargo xtask package` and `cargo xtask checksums`: the release's packages per platform and
//! the checksum list over all of them.

mod app_icon;
mod asset_names;
pub mod checksums;
mod linux_package;
mod macos_package;
mod release_platform;
mod staging;
mod tool_command;
mod windows_package;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::notices::repository_root;
use crate::set_version::{read_table, validate_version};
use linux_package::{LinuxInputs, package_linux};
use macos_package::{MacosInputs, package_macos};
use release_platform::ReleasePlatform;
use staging::{create_folder, recreate_folder};
use windows_package::package_windows;

pub const USAGE_LINE: &str = "cargo xtask package <platform> [--version <v>] --build-label <text> --out <folder> --binary <path> [--binary <path>] [--appimagetool <path> --appimage-runtime <path>]";

/// Where each platform's package is staged, under the repository root. It is kept after the
/// command, so the workflow can smoke-test the macOS bundle in place.
pub const PACKAGE_WORK_FOLDER: &str = "target/package";
const EDITOR_MANIFEST: &str = "crates/aurora-asset-editor/Cargo.toml";

/// The `package` command line, checked against what the platform needs.
#[derive(Debug, PartialEq, Eq)]
struct PackageOptions {
    platform: ReleasePlatform,
    version: Option<String>,
    build_label: String,
    out: PathBuf,
    /// One binary, or on macOS the arm64 binary then the x86_64 one.
    binaries: Vec<PathBuf>,
    appimage_tools: Option<AppImageTools>,
}

#[derive(Debug, PartialEq, Eq)]
struct AppImageTools {
    appimagetool: PathBuf,
    runtime: PathBuf,
}

/// Packages one platform's release binaries into `--out`, printing each file written.
pub fn run(arguments: &[&str]) -> Result<()> {
    let options = parse_options(arguments)?;
    let root = repository_root();
    let version = package_version(&root, options.version.as_deref())?;
    create_folder(&options.out)?;
    let work_folder = work_folder(&root, options.platform);
    recreate_folder(&work_folder)?;

    let written = match options.platform {
        ReleasePlatform::WindowsX86_64 => vec![package_windows(
            &options.binaries[0],
            &version,
            &options.out,
        )?],
        ReleasePlatform::LinuxX86_64 | ReleasePlatform::LinuxAarch64 => {
            let tools = options
                .appimage_tools
                .as_ref()
                .expect("parse_options requires the AppImage tools on Linux");
            package_linux(
                &root,
                &LinuxInputs {
                    platform: options.platform,
                    version: &version,
                    binary: &options.binaries[0],
                    appimagetool: &tools.appimagetool,
                    appimage_runtime: &tools.runtime,
                    work_folder: &work_folder,
                    out: &options.out,
                },
            )?
        }
        ReleasePlatform::MacosUniversal => package_macos(
            &root,
            &MacosInputs {
                version: &version,
                build_label: &options.build_label,
                arm64_binary: &options.binaries[0],
                x86_64_binary: &options.binaries[1],
                work_folder: &work_folder,
                out: &options.out,
            },
        )?,
    };
    for path in written {
        println!("Wrote {}", path.display());
    }
    Ok(())
}

pub fn work_folder(root: &Path, platform: ReleasePlatform) -> PathBuf {
    root.join(PACKAGE_WORK_FOLDER).join(platform.name())
}

fn parse_options(arguments: &[&str]) -> Result<PackageOptions> {
    let Some((platform, flags)) = arguments.split_first() else {
        bail!("usage: {USAGE_LINE}");
    };
    let platform = ReleasePlatform::from_name(platform)?;
    let mut version = None;
    let mut build_label = None;
    let mut out = None;
    let mut binaries = Vec::new();
    let mut appimagetool = None;
    let mut appimage_runtime = None;
    let mut remaining = flags.iter();
    while let Some(&flag) = remaining.next() {
        let Some(&value) = remaining.next() else {
            bail!("{flag} needs a value; usage: {USAGE_LINE}");
        };
        let slot = match flag {
            "--version" => &mut version,
            "--build-label" => &mut build_label,
            "--out" => &mut out,
            "--appimagetool" => &mut appimagetool,
            "--appimage-runtime" => &mut appimage_runtime,
            "--binary" => {
                binaries.push(PathBuf::from(value));
                continue;
            }
            other => bail!("unknown option `{other}`; usage: {USAGE_LINE}"),
        };
        if slot.replace(value.to_owned()).is_some() {
            bail!("{flag} is given twice");
        }
    }

    let Some(build_label) = build_label else {
        bail!("--build-label is required; usage: {USAGE_LINE}");
    };
    let Some(out) = out else {
        bail!("--out is required; usage: {USAGE_LINE}");
    };
    let binary_count = expected_binary_count(platform);
    if binaries.len() != binary_count {
        bail!(
            "{} takes {binary_count} --binary options, {} given",
            platform.name(),
            binaries.len()
        );
    }
    let appimage_tools = appimage_tools(platform, appimagetool, appimage_runtime)?;
    Ok(PackageOptions {
        platform,
        version,
        build_label,
        out: PathBuf::from(out),
        binaries,
        appimage_tools,
    })
}

fn expected_binary_count(platform: ReleasePlatform) -> usize {
    match platform {
        ReleasePlatform::MacosUniversal => 2,
        ReleasePlatform::WindowsX86_64
        | ReleasePlatform::LinuxX86_64
        | ReleasePlatform::LinuxAarch64 => 1,
    }
}

/// Linux needs both AppImage tools; the other platforms take neither.
fn appimage_tools(
    platform: ReleasePlatform,
    appimagetool: Option<String>,
    runtime: Option<String>,
) -> Result<Option<AppImageTools>> {
    let is_linux = matches!(
        platform,
        ReleasePlatform::LinuxX86_64 | ReleasePlatform::LinuxAarch64
    );
    match (is_linux, appimagetool, runtime) {
        (true, Some(appimagetool), Some(runtime)) => Ok(Some(AppImageTools {
            appimagetool: PathBuf::from(appimagetool),
            runtime: PathBuf::from(runtime),
        })),
        (true, _, _) => bail!(
            "{} needs --appimagetool and --appimage-runtime",
            platform.name()
        ),
        (false, None, None) => Ok(None),
        (false, _, _) => bail!(
            "{} builds no AppImage, so it takes no --appimagetool or --appimage-runtime",
            platform.name()
        ),
    }
}

/// `requested`, or the editor package's own version when none is given.
fn package_version(root: &Path, requested: Option<&str>) -> Result<String> {
    if let Some(version) = requested {
        validate_version(version)?;
        return Ok(version.to_owned());
    }
    editor_version(root)
}

/// The editor's `package.version`, following `version.workspace = true` to the workspace
/// manifest.
fn editor_version(root: &Path) -> Result<String> {
    let manifest = read_table(&root.join(EDITOR_MANIFEST))?;
    let version = manifest
        .get("package")
        .and_then(|package| package.get("version"))
        .with_context(|| format!("{EDITOR_MANIFEST} has no package.version"))?;
    if let Some(version) = version.as_str() {
        return Ok(version.to_owned());
    }
    let inherits = version.get("workspace").and_then(toml::Value::as_bool) == Some(true);
    if !inherits {
        bail!("{EDITOR_MANIFEST} has a package.version that is neither text nor inherited");
    }
    let workspace = read_table(&root.join("Cargo.toml"))?;
    workspace
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .context("Cargo.toml has no workspace.package.version")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(arguments: &str) -> Result<PackageOptions> {
        let arguments: Vec<&str> = arguments.split(' ').collect();
        parse_options(&arguments)
    }

    #[test]
    fn package_version_defaults_to_the_editor_version() {
        let root = repository_root();
        let workspace = read_table(&root.join("Cargo.toml")).unwrap();
        let workspace_version = workspace["workspace"]["package"]["version"]
            .as_str()
            .unwrap();

        assert_eq!(package_version(&root, None).unwrap(), workspace_version);
        assert_eq!(package_version(&root, Some("2.0.7")).unwrap(), "2.0.7");
        assert!(package_version(&root, Some("2.0")).is_err());
    }

    #[test]
    fn package_options_follow_the_platform() {
        let macos = options(
            "macos-universal --build-label label --out dist --binary arm64 --binary x86_64",
        )
        .unwrap();
        assert_eq!(
            macos.binaries,
            [PathBuf::from("arm64"), PathBuf::from("x86_64")]
        );
        assert_eq!(macos.version, None);

        let linux = options(
            "linux-x86_64 --version 2.0.7 --build-label label --out dist --binary app --appimagetool tool --appimage-runtime runtime",
        )
        .unwrap();
        assert_eq!(linux.version.as_deref(), Some("2.0.7"));
        assert_eq!(
            linux.appimage_tools,
            Some(AppImageTools {
                appimagetool: PathBuf::from("tool"),
                runtime: PathBuf::from("runtime"),
            })
        );

        for refused in [
            "macos-universal --build-label label --out dist --binary arm64",
            "windows-x86_64 --build-label label --out dist --binary a --binary b",
            "linux-x86_64 --build-label label --out dist --binary app --appimagetool tool",
            "windows-x86_64 --build-label label --out dist --binary app --appimagetool tool --appimage-runtime runtime",
            "windows-x86_64 --out dist --binary app",
            "windows-x86_64 --build-label label --binary app",
            "windows-x86_64 --build-label label --out dist --out other --binary app",
            "windows-x86_64 --build-label label --out dist --binary app --colour blue",
            "windows-x86_64 --build-label label --out dist --binary",
        ] {
            assert!(options(refused).is_err(), "{refused}");
        }
    }

    #[test]
    fn macos_bundle_binary_is_in_the_macos_work_folder() {
        let root = Path::new("/repo");

        assert!(
            root.join(macos_package::MACOS_BUNDLE_BINARY)
                .starts_with(work_folder(root, ReleasePlatform::MacosUniversal))
        );
    }
}
