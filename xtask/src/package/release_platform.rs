use anyhow::{Result, bail};

/// A platform the release ships a package for. The names are the ones the asset names and the
/// workflow use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleasePlatform {
    WindowsX86_64,
    LinuxX86_64,
    LinuxAarch64,
    MacosUniversal,
}

impl ReleasePlatform {
    pub const ALL: [ReleasePlatform; 4] = [
        ReleasePlatform::WindowsX86_64,
        ReleasePlatform::LinuxX86_64,
        ReleasePlatform::LinuxAarch64,
        ReleasePlatform::MacosUniversal,
    ];

    pub fn from_name(name: &str) -> Result<ReleasePlatform> {
        match Self::ALL
            .into_iter()
            .find(|platform| platform.name() == name)
        {
            Some(platform) => Ok(platform),
            None => {
                let names: Vec<&str> = Self::ALL.iter().map(|platform| platform.name()).collect();
                bail!(
                    "`{name}` is not a release platform; use one of {}",
                    names.join(", ")
                )
            }
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ReleasePlatform::WindowsX86_64 => "windows-x86_64",
            ReleasePlatform::LinuxX86_64 => "linux-x86_64",
            ReleasePlatform::LinuxAarch64 => "linux-aarch64",
            ReleasePlatform::MacosUniversal => "macos-universal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_names_round_trip() {
        for platform in ReleasePlatform::ALL {
            assert_eq!(
                ReleasePlatform::from_name(platform.name()).unwrap(),
                platform
            );
        }
    }

    #[test]
    fn unknown_platform_is_refused() {
        let message = ReleasePlatform::from_name("macos-aarch64")
            .unwrap_err()
            .to_string();

        assert!(message.contains("`macos-aarch64`"), "{message}");
        for name in [
            "windows-x86_64",
            "linux-x86_64",
            "linux-aarch64",
            "macos-universal",
        ] {
            assert!(message.contains(name), "{message}");
        }
    }
}
