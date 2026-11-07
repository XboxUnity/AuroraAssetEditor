mod ftp_settings;

use directories::BaseDirs;
use std::io;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::path::{Path, PathBuf};

pub use ftp_settings::{FTP_SETTINGS_FILE, FtpSettings, SettingsError};

const APP_FOLDER_NAME: &str = "AuroraAssetEditor";
// TEST-NET-1: never routed, and connecting a UDP socket sends nothing.
const ROUTE_PROBE_ADDRESS: &str = "192.0.2.1:9";

pub fn app_config_dir() -> Option<PathBuf> {
    BaseDirs::new().map(|base| config_dir_in(base.config_dir()))
}

pub(crate) fn config_dir_in(config_home: &Path) -> PathBuf {
    config_home.join(APP_FOLDER_NAME)
}

pub fn local_subnet_prefix() -> io::Result<String> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(ROUTE_PROBE_ADDRESS)?;
    prefix_of_local_address(socket.local_addr()?.ip())
}

pub(crate) fn prefix_of_local_address(address: IpAddr) -> io::Result<String> {
    match address {
        IpAddr::V4(v4) if !v4.is_unspecified() => Ok(subnet_prefix(v4)),
        _ => Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            format!("no usable IPv4 address on the default route (got {address})"),
        )),
    }
}

pub(crate) fn subnet_prefix(address: Ipv4Addr) -> String {
    let [a, b, c, _] = address.octets();
    format!("{a}.{b}.{c}.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_joins_app_folder() {
        let home = Path::new("C:\\Users\\x\\AppData\\Roaming");
        assert_eq!(config_dir_in(home), home.join("AuroraAssetEditor"));
    }

    #[test]
    fn subnet_prefix_keeps_three_octets() {
        assert_eq!(subnet_prefix(Ipv4Addr::new(192, 168, 1, 50)), "192.168.1.");
    }

    #[test]
    fn unusable_local_address_is_error() {
        assert!(prefix_of_local_address(IpAddr::V4(Ipv4Addr::UNSPECIFIED)).is_err());
        assert!(prefix_of_local_address(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)).is_err());
        assert_eq!(
            prefix_of_local_address(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7))).unwrap(),
            "10.0.0."
        );
    }
}
