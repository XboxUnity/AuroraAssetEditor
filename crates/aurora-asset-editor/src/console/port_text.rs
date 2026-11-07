use aurora_console::FtpSettings;
use dotnet_number_text::trim_number_text;

/// The port text as the old app saved it: text that is not an `i32` becomes "21", anything else (a 0
/// included) is kept as typed. This is the `i32` half of `FtpSettings::port_number`, which also maps 0 to
/// 21 when connecting.
pub fn port_text_for_save(text: &str) -> String {
    match trim_number_text(text).parse::<i32>() {
        Ok(_) => text.to_owned(),
        Err(_) => FtpSettings::default().port,
    }
}
