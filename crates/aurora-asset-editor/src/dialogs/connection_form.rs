use aurora_console::FtpSettings;

use crate::console::port_text_for_save;
use crate::status::Severity;

/// The connection dialog's boxes and the result of the last test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionForm {
    pub host: String,
    pub port: String,
    pub username: String,
    pub password: String,
    pub test_status: Option<(Severity, String)>,
}

impl ConnectionForm {
    /// Opens with the session's settings; without any, with the defaults and the subnet prefix as the host,
    /// as the old FTP page did.
    pub fn new(settings: Option<&FtpSettings>, subnet_prefix: &str) -> ConnectionForm {
        let settings = match settings {
            Some(settings) => settings.clone(),
            None => FtpSettings {
                host: subnet_prefix.to_owned(),
                ..FtpSettings::default()
            },
        };
        ConnectionForm {
            host: settings.host,
            port: settings.port,
            username: settings.username,
            password: settings.password,
            test_status: None,
        }
    }

    /// The settings the boxes hold, as a test or a save uses them.
    pub fn to_settings(&self) -> FtpSettings {
        FtpSettings {
            host: self.host.clone(),
            port: port_text_for_save(&self.port),
            username: self.username.clone(),
            password: self.password.clone(),
        }
    }
}
