use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::app_icon::{load_app_icon, write_png};
use super::asset_names::{APPIMAGE_EXTENSION, TARBALL_EXTENSION, asset_name, asset_stem};
use super::release_platform::ReleasePlatform;
use super::staging::{copy_file, create_folder, make_executable, symlink, write_text};
use super::tool_command::{ToolCommand, path_argument};

/// The newest glibc the Linux binaries may need: Ubuntu 22.04's, which Debian 12 also covers.
pub const GLIBC_FLOOR: (u32, u32) = (2, 35);
const GLIBC_PREFIX: &str = "GLIBC_";

const BINARY_NAME: &str = "aurora-asset-editor";
const LICENSE_FILE: &str = "UNLICENSE";
const APP_DIR: &str = "AppDir";
const APP_RUN: &str = "AppRun";
const APP_DIR_BINARY: &str = "usr/bin/aurora-asset-editor";
const DESKTOP_FILE: &str = "aurora-asset-editor.desktop";
const ICON_FILE: &str = "aurora-asset-editor.png";

/// What `package` hands the Linux packaging.
pub struct LinuxInputs<'a> {
    pub platform: ReleasePlatform,
    pub version: &'a str,
    pub binary: &'a Path,
    pub appimagetool: &'a Path,
    pub appimage_runtime: &'a Path,
    pub work_folder: &'a Path,
    pub out: &'a Path,
}

/// Where the Linux packaging reads, stages and writes.
struct LinuxPaths {
    binary: PathBuf,
    work_folder: PathBuf,
    /// The tarball's single top-level folder, `AuroraAssetEditor-<v>-<platform>`.
    tar_folder_name: String,
    tarball: PathBuf,
    app_dir: PathBuf,
    appimage: PathBuf,
    appimagetool: PathBuf,
    appimage_runtime: PathBuf,
    /// The architecture name appimagetool expects.
    appimage_arch: &'static str,
}

impl LinuxPaths {
    fn new(inputs: &LinuxInputs) -> Result<LinuxPaths> {
        let appimage_arch = match inputs.platform {
            ReleasePlatform::LinuxX86_64 => "x86_64",
            ReleasePlatform::LinuxAarch64 => "aarch64",
            other => bail!("{} is not a Linux platform", other.name()),
        };
        let (platform, version) = (inputs.platform, inputs.version);
        Ok(LinuxPaths {
            binary: inputs.binary.to_path_buf(),
            work_folder: inputs.work_folder.to_path_buf(),
            tar_folder_name: asset_stem(platform, version),
            tarball: inputs
                .out
                .join(asset_name(platform, version, TARBALL_EXTENSION)),
            app_dir: inputs.work_folder.join(APP_DIR),
            appimage: inputs
                .out
                .join(asset_name(platform, version, APPIMAGE_EXTENSION)),
            appimagetool: inputs.appimagetool.to_path_buf(),
            appimage_runtime: inputs.appimage_runtime.to_path_buf(),
            appimage_arch,
        })
    }
}

/// The tools the Linux packaging runs, in order: the glibc symbol listing, the tarball, the
/// AppImage.
fn linux_commands(paths: &LinuxPaths) -> Vec<ToolCommand> {
    vec![
        ToolCommand::new("objdump", ["-T".to_owned(), path_argument(&paths.binary)]),
        ToolCommand::new(
            "tar",
            [
                "-czf".to_owned(),
                path_argument(&paths.tarball),
                "-C".to_owned(),
                path_argument(&paths.work_folder),
                paths.tar_folder_name.clone(),
            ],
        ),
        ToolCommand::new(
            &path_argument(&paths.appimagetool),
            [
                "--runtime-file".to_owned(),
                path_argument(&paths.appimage_runtime),
                path_argument(&paths.app_dir),
                path_argument(&paths.appimage),
            ],
        )
        .with_environment("ARCH", paths.appimage_arch)
        // Runners have no FUSE, so appimagetool (itself an AppImage) unpacks itself to run.
        .with_environment("APPIMAGE_EXTRACT_AND_RUN", "1"),
    ]
}

/// Writes the tarball and the AppImage into the output folder, after checking the binary
/// against the glibc floor. Returns the written files.
pub fn package_linux(root: &Path, inputs: &LinuxInputs) -> Result<Vec<PathBuf>> {
    let paths = LinuxPaths::new(inputs)?;
    let [list_symbols, create_tarball, create_appimage] =
        <[ToolCommand; 3]>::try_from(linux_commands(&paths))
            .expect("linux_commands builds three commands");

    let symbols = list_symbols.run()?;
    check_glibc_floor(&String::from_utf8_lossy(&symbols.stdout))?;

    stage_tar_folder(root, &paths)?;
    create_tarball.run()?;

    stage_app_dir(root, &paths)?;
    make_executable(&paths.appimagetool)?;
    create_appimage.run()?;

    Ok(vec![paths.appimage, paths.tarball])
}

fn stage_tar_folder(root: &Path, paths: &LinuxPaths) -> Result<()> {
    let folder = paths.work_folder.join(&paths.tar_folder_name);
    create_folder(&folder)?;
    let binary = folder.join(BINARY_NAME);
    copy_file(&paths.binary, &binary)?;
    make_executable(&binary)?;
    copy_file(&root.join(LICENSE_FILE), &folder.join(LICENSE_FILE))
}

fn stage_app_dir(root: &Path, paths: &LinuxPaths) -> Result<()> {
    let binary = paths.app_dir.join(APP_DIR_BINARY);
    let binary_folder = binary.parent().expect("the AppDir binary sits in usr/bin");
    create_folder(binary_folder)?;
    copy_file(&paths.binary, &binary)?;
    make_executable(&binary)?;
    symlink(APP_DIR_BINARY, &paths.app_dir.join(APP_RUN))?;
    write_text(&paths.app_dir.join(DESKTOP_FILE), &desktop_entry())?;
    write_png(&load_app_icon(root)?, &paths.app_dir.join(ICON_FILE))
}

pub fn desktop_entry() -> String {
    [
        "[Desktop Entry]",
        "Type=Application",
        "Name=Aurora Asset Editor",
        "Exec=aurora-asset-editor %F",
        "Icon=aurora-asset-editor",
        "Categories=Graphics;",
        "Terminal=false",
        "",
    ]
    .join("\n")
}

/// The newest `GLIBC_<major>.<minor>` symbol version in `objdump -T` output.
pub fn highest_glibc(objdump_output: &str) -> Option<(u32, u32)> {
    objdump_output
        .split(GLIBC_PREFIX)
        .skip(1)
        .filter_map(leading_version)
        .max()
}

/// The `<major>.<minor>` at the start of `text`; a third number, as in `2.2.5`, is ignored.
fn leading_version(text: &str) -> Option<(u32, u32)> {
    let version_length = text
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(text.len());
    let mut numbers = text[..version_length].split('.');
    let major = numbers.next()?.parse().ok()?;
    let minor = numbers.next()?.parse().ok()?;
    Some((major, minor))
}

/// Fails when the binary needs a glibc newer than `GLIBC_FLOOR`, naming the version it needs.
pub fn check_glibc_floor(objdump_output: &str) -> Result<()> {
    let Some(needed) = highest_glibc(objdump_output) else {
        return Ok(());
    };
    if needed > GLIBC_FLOOR {
        let (major, minor) = needed;
        let (floor_major, floor_minor) = GLIBC_FLOOR;
        bail!(
            "the binary needs {GLIBC_PREFIX}{major}.{minor}, newer than the floor {GLIBC_PREFIX}{floor_major}.{floor_minor}, so it would not start on older Linux systems; build it against glibc {floor_major}.{floor_minor}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OBJDUMP_LINES: &str = "\
target/release/aurora-asset-editor:     file format elf64-x86-64

DYNAMIC SYMBOL TABLE:
0000000000000000      DF *UND*\t0000000000000000 (GLIBC_2.2.5) free
0000000000000000      DF *UND*\t0000000000000000 (GLIBC_2.17) clock_gettime
0000000000000000      DF *UND*\t0000000000000000 (GLIBC_2.34) __libc_start_main
0000000000000000      DF *UND*\t0000000000000000  GLIBC_2.35  __rseq_offset
0000000000000000      DO *UND*\t0000000000000000 (GLIBC_PRIVATE) _rtld_global
0000000000000000  w   D  *UND*\t0000000000000000  Base        __gmon_start__
";

    #[test]
    fn desktop_entry_has_the_required_keys() {
        let entry = desktop_entry();
        let lines: Vec<&str> = entry.lines().collect();

        assert_eq!(lines[0], "[Desktop Entry]");
        for line in [
            "Type=Application",
            "Name=Aurora Asset Editor",
            "Exec=aurora-asset-editor %F",
            "Icon=aurora-asset-editor",
            "Categories=Graphics;",
            "Terminal=false",
        ] {
            assert!(lines.contains(&line), "{line} missing from\n{entry}");
        }
    }

    #[test]
    fn glibc_floor_is_checked_from_objdump_output() {
        assert_eq!(highest_glibc(OBJDUMP_LINES), Some((2, 35)));
        assert!(check_glibc_floor(OBJDUMP_LINES).is_ok());

        let too_new = format!(
            "{OBJDUMP_LINES}0000000000000000      DF *UND*\t0000000000000000 (GLIBC_2.39) pidfd_spawnp\n"
        );
        let message = check_glibc_floor(&too_new).unwrap_err().to_string();
        assert!(message.contains("GLIBC_2.39"), "{message}");
        assert!(message.contains("GLIBC_2.35"), "{message}");

        let without_glibc = "DYNAMIC SYMBOL TABLE:\n0000000000000000  w   D  *UND*\t0000000000000000  Base  __gmon_start__\n";
        assert_eq!(highest_glibc(without_glibc), None);
        assert!(check_glibc_floor(without_glibc).is_ok());
    }

    #[test]
    fn linux_commands_check_glibc_then_pack_the_tarball_and_the_appimage() {
        let work = Path::new("/work/target/package/linux-aarch64");
        let out = Path::new("/work/dist");
        let inputs = LinuxInputs {
            platform: ReleasePlatform::LinuxAarch64,
            version: "2.0.7",
            binary: Path::new("/work/target/release/aurora-asset-editor"),
            appimagetool: Path::new("/tools/appimagetool"),
            appimage_runtime: Path::new("/tools/runtime-aarch64"),
            work_folder: work,
            out,
        };

        let commands = linux_commands(&LinuxPaths::new(&inputs).unwrap());

        let text = |path: &Path| path_argument(path);
        assert_eq!(
            commands,
            [
                ToolCommand::new(
                    "objdump",
                    ["-T", "/work/target/release/aurora-asset-editor"]
                ),
                ToolCommand::new(
                    "tar",
                    [
                        "-czf".to_owned(),
                        text(&out.join("AuroraAssetEditor-2.0.7-linux-aarch64.tar.gz")),
                        "-C".to_owned(),
                        text(work),
                        "AuroraAssetEditor-2.0.7-linux-aarch64".to_owned(),
                    ]
                ),
                ToolCommand::new(
                    "/tools/appimagetool",
                    [
                        "--runtime-file".to_owned(),
                        "/tools/runtime-aarch64".to_owned(),
                        text(&work.join("AppDir")),
                        text(&out.join("AuroraAssetEditor-2.0.7-linux-aarch64.AppImage")),
                    ]
                )
                .with_environment("ARCH", "aarch64")
                .with_environment("APPIMAGE_EXTRACT_AND_RUN", "1"),
            ]
        );
    }

    #[test]
    fn linux_packaging_refuses_other_platforms() {
        let inputs = LinuxInputs {
            platform: ReleasePlatform::MacosUniversal,
            version: "2.0.7",
            binary: Path::new("a"),
            appimagetool: Path::new("b"),
            appimage_runtime: Path::new("c"),
            work_folder: Path::new("d"),
            out: Path::new("e"),
        };

        assert!(LinuxPaths::new(&inputs).is_err());
    }
}
