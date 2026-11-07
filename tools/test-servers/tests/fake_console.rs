use std::io::{BufRead, BufReader, Cursor, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::str::FromStr;
use std::time::Duration;

use suppaftp::list::File;
use suppaftp::types::FileType;
use suppaftp::{FtpError, FtpStream, Status};
use test_servers::{FakeConsole, FakeConsoleConfig, FakeFeatures, ListingStyle};

const ASSET_FOLDER: &str = "/Game/Data/GameData/AB_CD";
const ASSET_PATH: &str = "/Game/Data/GameData/AB_CD/GC1.asset";

fn connect(console: &FakeConsole) -> FtpStream {
    FtpStream::connect(("127.0.0.1", console.port())).expect("connecting to the fake console")
}

fn logged_in(console: &FakeConsole) -> FtpStream {
    let mut ftp = connect(console);
    ftp.login("xboxftp", "xboxftp")
        .expect("logging in with the default credentials");
    ftp.transfer_type(FileType::Binary)
        .expect("switching to binary transfers");
    ftp
}

fn reply_text(result: Result<suppaftp::types::Response, FtpError>) -> String {
    let body = match result {
        Ok(response) => response.body,
        Err(FtpError::UnexpectedResponse(response)) => response.body,
        Err(other) => panic!("expected a reply from the server, got {other}"),
    };
    String::from_utf8(body).expect("replies are UTF-8")
}

fn config_with(features: FakeFeatures) -> FakeConsoleConfig {
    FakeConsoleConfig {
        features,
        ..FakeConsoleConfig::default()
    }
}

#[test]
fn logs_in_and_answers_site_revision() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut ftp = logged_in(&console);

    let reply = reply_text(ftp.custom_command("SITE REVISION", &[Status::CommandOk]));
    assert_eq!(reply.trim_end(), "200 1655");

    let mut wrong = connect(&console);
    let Err(FtpError::UnexpectedResponse(response)) = wrong.login("xboxftp", "wrong") else {
        panic!("a wrong password must be refused");
    };
    assert!(String::from_utf8_lossy(&response.body).starts_with("530"));
}

#[test]
fn feat_follows_configuration() {
    let advertised = |features| {
        let console = FakeConsole::start(config_with(features));
        let mut ftp = logged_in(&console);
        ftp.feat().map(|features| {
            let mut names: Vec<String> = features.into_keys().collect();
            names.sort();
            names
        })
    };

    assert_eq!(advertised(FakeFeatures::Basic).unwrap(), ["SIZE"]);
    assert_eq!(
        advertised(FakeFeatures::MlsdAndUtf8).unwrap(),
        ["MLSD", "UTF8"]
    );
    let Err(FtpError::UnexpectedResponse(response)) = advertised(FakeFeatures::Refused) else {
        panic!("a refused FEAT must come back as an error reply");
    };
    assert!(String::from_utf8_lossy(&response.body).starts_with("500"));

    let console = FakeConsole::start(config_with(FakeFeatures::MlsdAndUtf8));
    let mut ftp = logged_in(&console);
    let reply = reply_text(ftp.custom_command("OPTS UTF8 ON", &[Status::CommandOk]));
    assert!(reply.starts_with("200"), "got {reply:?}");
}

#[test]
fn cwd_and_retr_ignore_case() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let bytes: Vec<u8> = (0..=255).collect();
    console.add_file(ASSET_PATH, &bytes);
    let mut ftp = logged_in(&console);

    ftp.cwd("/game/data/gamedata/ab_cd")
        .expect("CWD ignores case");
    let downloaded = ftp
        .retr_as_buffer("gc1.ASSET")
        .expect("RETR ignores case")
        .into_inner();

    assert_eq!(downloaded, bytes);
}

#[test]
fn stor_replaces_bytes_and_keeps_name() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    console.add_file(ASSET_PATH, &[7; 50]);
    let mut ftp = logged_in(&console);
    ftp.cwd(ASSET_FOLDER).expect("the folder exists");

    let new_bytes = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    ftp.put_file("gc1.asset", &mut Cursor::new(new_bytes))
        .expect("STOR over an existing file");
    let listing = ftp.list(None).expect("listing the folder");

    assert_eq!(console.file(ASSET_PATH), Some(new_bytes.to_vec()));
    assert_eq!(listing.len(), 1);
    assert!(listing[0].ends_with(" GC1.asset"), "got {:?}", listing[0]);
}

#[test]
fn missing_folder_is_550() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut ftp = logged_in(&console);

    let Err(FtpError::UnexpectedResponse(response)) = ftp.cwd("/Game/Nowhere") else {
        panic!("CWD to an unknown folder must be refused");
    };

    assert!(String::from_utf8_lossy(&response.body).starts_with("550"));
}

#[test]
fn listing_styles() {
    let parse_line = |style: ListingStyle, command: &str| {
        let console = FakeConsole::start(FakeConsoleConfig {
            features: FakeFeatures::MlsdAndUtf8,
            listing_style: style,
            ..FakeConsoleConfig::default()
        });
        console.add_file("/Game/Data/Content.db", &[0; 2048]);
        let mut ftp = logged_in(&console);
        ftp.cwd("/Game/Data").expect("the folder exists");
        let lines = match command {
            "LIST" => ftp.list(None),
            _ => ftp.mlsd(None),
        }
        .expect("listing the folder");
        assert_eq!(lines.len(), 1, "got {lines:?}");
        lines.into_iter().next().expect("one line")
    };

    let unix = File::from_str(&parse_line(ListingStyle::Unix, "LIST")).expect("a Unix line");
    let dos = File::from_str(&parse_line(ListingStyle::Dos, "LIST")).expect("a DOS line");
    let mlsd_line = parse_line(ListingStyle::Mlsd, "MLSD");
    #[allow(deprecated)]
    let mlsd = File::from_mlsx_line(&mlsd_line).expect("an MLSD line");
    let mlsd_list_line = parse_line(ListingStyle::Mlsd, "LIST");
    let mlsd_list = File::from_str(&mlsd_list_line).expect("LIST stays Unix in Mlsd style");

    for file in [unix, dos, mlsd, mlsd_list] {
        assert_eq!(file.name(), "Content.db");
        assert_eq!(file.size(), 2048);
    }
}

#[test]
fn advertised_mlsd_answers_in_machine_format_whatever_the_list_style() {
    let console = FakeConsole::start(FakeConsoleConfig {
        features: FakeFeatures::MlsdAndUtf8,
        listing_style: ListingStyle::Unix,
        ..FakeConsoleConfig::default()
    });
    console.add_file("/Game/Data/Content.db", &[0; 2048]);
    let mut ftp = logged_in(&console);
    ftp.cwd("/Game/Data").expect("the folder exists");

    let lines = ftp.mlsd(None).expect("MLSD is advertised, so it works");

    assert_eq!(lines.len(), 1, "got {lines:?}");
    assert!(
        lines[0].starts_with("type=file;size=2048;"),
        "got {:?}",
        lines[0]
    );
}

#[test]
fn site_answers_only_the_revision_command() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut ftp = logged_in(&console);

    let revision = reply_text(ftp.custom_command("site revision", &[Status::CommandOk]));
    let other = reply_text(ftp.custom_command("SITE HELP", &[Status::CommandOk]));

    assert_eq!(revision.trim_end(), "200 1655");
    assert!(other.starts_with("502"), "got {other:?}");
}

#[test]
fn truncated_download_sends_fewer_bytes() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    console.add_file(ASSET_PATH, &[9; 2048]);
    console.truncate_download(ASSET_PATH, 100);
    let mut ftp = logged_in(&console);

    let downloaded = ftp
        .retr_as_buffer(ASSET_PATH)
        .expect("the truncated download still ends with 226")
        .into_inner();

    assert_eq!(downloaded.len(), 100);
}

#[test]
fn drop_connections_closes_the_session() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut ftp = logged_in(&console);
    ftp.noop().expect("the session works before the drop");

    console.drop_connections();

    assert!(matches!(ftp.noop(), Err(FtpError::ConnectionError(_))));
}

#[test]
fn time_out_next_command_answers_421_and_closes() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut ftp = logged_in(&console);

    console.time_out_next_command();

    let reply = reply_text(ftp.custom_command("NOOP", &[Status::CommandOk]));
    assert!(reply.starts_with("421"), "got {reply:?}");
    assert!(ftp.noop().is_err());
    let mut fresh = logged_in(&console);
    fresh.noop().expect("later commands are unaffected");
}

#[test]
fn refuse_new_connections_rejects_new_logins() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut open_session = logged_in(&console);

    console.refuse_new_connections();

    assert!(matches!(
        FtpStream::connect(("127.0.0.1", console.port())),
        Err(FtpError::ConnectionError(_))
    ));
    open_session
        .noop()
        .expect("a session opened earlier keeps working");
}

#[test]
fn commands_are_logged_with_password_masked() {
    let console = FakeConsole::start(FakeConsoleConfig {
        password: "s3cret-pass".to_owned(),
        ..FakeConsoleConfig::default()
    });
    let mut ftp = connect(&console);
    ftp.login("xboxftp", "s3cret-pass")
        .expect("logging in with the configured password");

    let commands = console.commands();

    assert!(commands.contains(&"USER xboxftp".to_owned()));
    assert!(commands.contains(&"PASS ****".to_owned()));
    assert!(
        commands
            .iter()
            .all(|command| !command.contains("s3cret-pass"))
    );
    assert_eq!(console.login_count(), 1);
}

#[test]
fn dropping_the_console_stops_its_listener() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let port = console.port();
    let _open_session = logged_in(&console);

    drop(console);

    assert!(FtpStream::connect(("127.0.0.1", port)).is_err());
}

fn read_reply(control: &mut BufReader<TcpStream>) -> String {
    let mut reply = String::new();
    control.read_line(&mut reply).expect("reading a reply");
    reply
}

fn send_command(control: &mut BufReader<TcpStream>, command: &str) -> String {
    control
        .get_mut()
        .write_all(format!("{command}\r\n").as_bytes())
        .expect("sending a command");
    read_reply(control)
}

fn passive_port(reply: &str) -> u16 {
    let numbers: Vec<u16> = reply
        .trim_end()
        .trim_end_matches(')')
        .rsplit('(')
        .next()
        .expect("a parenthesised address")
        .split(',')
        .map(|part| part.parse().expect("a number"))
        .collect();
    numbers[4] * 256 + numbers[5]
}

#[test]
fn stalled_download_goes_silent_after_the_given_bytes() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    console.add_file(ASSET_PATH, &[5; 1000]);
    console.stall_download(ASSET_PATH, 40);
    let stream = TcpStream::connect(("127.0.0.1", console.port())).expect("connecting");
    let mut control = BufReader::new(stream);
    assert!(read_reply(&mut control).starts_with("220"));
    assert!(send_command(&mut control, "USER xboxftp").starts_with("331"));
    assert!(send_command(&mut control, "PASS xboxftp").starts_with("230"));
    let pasv = send_command(&mut control, "PASV");
    let mut data = TcpStream::connect(("127.0.0.1", passive_port(&pasv))).expect("data connection");
    data.set_read_timeout(Some(Duration::from_millis(300)))
        .expect("setting a read timeout");
    assert!(send_command(&mut control, &format!("RETR {ASSET_PATH}")).starts_with("150"));

    let mut received = [0u8; 40];
    data.read_exact(&mut received)
        .expect("the first 40 bytes arrive");
    let silence = data.read(&mut [0u8; 1]).expect_err("nothing more is sent");

    assert!(matches!(
        silence.kind(),
        ErrorKind::WouldBlock | ErrorKind::TimedOut
    ));
}
