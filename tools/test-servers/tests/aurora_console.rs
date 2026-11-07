//! The fake console's default behaviour, checked on the wire against what Aurora's FtpDll sends
//! (see `.superpowers/sdd/cleanroom-ftp/aurora-ftp-server.md`).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

use test_servers::{
    FAKE_INPUT_HASH, FAKE_STORED_HASH, FakeConsole, FakeConsoleConfig, FileDate, ListingStyle,
};

struct RawSession {
    control: BufReader<TcpStream>,
}

impl RawSession {
    fn connect(console: &FakeConsole) -> RawSession {
        let stream = TcpStream::connect(("127.0.0.1", console.port())).expect("connecting");
        RawSession {
            control: BufReader::new(stream),
        }
    }

    fn logged_in(console: &FakeConsole) -> RawSession {
        let mut session = RawSession::connect(console);
        assert_eq!(session.read_line(), "220 FtpDll Ready\r\n");
        assert_eq!(
            session.send("USER xboxftp"),
            "331 User xboxftp OK, need password\r\n"
        );
        assert_eq!(session.send("PASS xboxftp"), "230 User logged in\r\n");
        session
    }

    fn read_line(&mut self) -> String {
        let mut line = String::new();
        self.control.read_line(&mut line).expect("reading a reply");
        line
    }

    fn send(&mut self, command: &str) -> String {
        self.control
            .get_mut()
            .write_all(format!("{command}\r\n").as_bytes())
            .expect("sending a command");
        self.read_line()
    }

    /// A multi-line reply: lines up to and including the one that starts with `last_prefix`.
    fn send_multiline(&mut self, command: &str, last_prefix: &str) -> Vec<String> {
        let mut lines = vec![self.send(command)];
        while !lines.last().expect("one line").starts_with(last_prefix) {
            lines.push(self.read_line());
        }
        lines
    }

    fn passive_port(&mut self) -> u16 {
        let reply = self.send("PASV");
        let numbers: Vec<u16> = reply
            .trim_end()
            .trim_end_matches(')')
            .rsplit('(')
            .next()
            .expect("a parenthesised address")
            .split(',')
            .map(|part| part.parse().expect("a number"))
            .collect();
        assert!(reply.starts_with("227 Entering Passive Mode (127,0,0,1,"));
        numbers[4] * 256 + numbers[5]
    }

    /// Runs a transfer command that sends data: the preliminary reply, the data and the final reply.
    fn download(&mut self, command: &str) -> (String, String, String) {
        let port = self.passive_port();
        let mut data = TcpStream::connect(("127.0.0.1", port)).expect("data connection");
        let preliminary = self.send(command);
        let mut received = Vec::new();
        data.read_to_end(&mut received).expect("reading the data");
        let completion = self.read_line();
        (
            preliminary,
            String::from_utf8(received).expect("UTF-8 data"),
            completion,
        )
    }
}

fn aurora_console() -> FakeConsole {
    FakeConsole::start(FakeConsoleConfig::default())
}

#[test]
fn default_config_is_aurora() {
    let config = FakeConsoleConfig::default();

    assert_eq!(config.listing_style, ListingStyle::Aurora);
    assert_eq!(config.site_revision_reply.as_deref(), Some("200 1655"));
}

#[test]
fn login_replies_follow_aurora() {
    let console = aurora_console();

    let mut wrong_user = RawSession::connect(&console);
    assert_eq!(wrong_user.read_line(), "220 FtpDll Ready\r\n");
    assert_eq!(wrong_user.send("USER nobody"), "332 User not OK\r\n");
    assert_eq!(wrong_user.send("PASS xboxftp"), "530 Need username\r\n");

    let mut wrong_password = RawSession::connect(&console);
    wrong_password.read_line();
    wrong_password.send("USER XBOXFTP");
    assert_eq!(
        wrong_password.send("PASS nope"),
        format!("530 Login failed: input = {FAKE_INPUT_HASH}, pass = {FAKE_STORED_HASH}\r\n")
    );
    assert_eq!(wrong_password.send("PASS xboxftp"), "530 Need username\r\n");

    let mut anonymous = RawSession::connect(&console);
    anonymous.read_line();
    assert_eq!(anonymous.send("CWD /"), "530 Not logged in\r\n");
    assert_eq!(anonymous.send("MLSD"), "530 Not logged in\r\n");

    let mut session = RawSession::logged_in(&console);
    assert_eq!(session.send("USER xboxftp"), "530 Already logged in\r\n");
    assert_eq!(session.send("PASS xboxftp"), "530 Already logged in\r\n");
}

#[test]
fn feat_site_and_simple_replies_follow_aurora() {
    let console = aurora_console();
    let mut session = RawSession::logged_in(&console);

    assert_eq!(
        session.send_multiline("FEAT", "211 "),
        [
            "211-Extensions supported:\r\n",
            " XCRC filename\r\n",
            " CDUP\r\n",
            " UTF8\r\n",
            "211 END\r\n"
        ]
    );
    assert_eq!(session.send("SITE REVISION"), "200 1655\r\n");
    assert_eq!(
        session.send("SITE HELP"),
        "502 Command SITE not implemented for this argument.\r\n"
    );
    assert_eq!(session.send("NOOP"), "200 ori r0, r0, 0\r\n");
    assert_eq!(session.send("TYPE I"), "200 Type set to I\r\n");
    assert_eq!(session.send("PWD"), "257 \"/\"\r\n");
    assert_eq!(session.send("QUIT"), "221 Goodbye\r\n");
}

#[test]
fn commands_aurora_lacks_answer_502() {
    let console = aurora_console();
    let mut session = RawSession::logged_in(&console);

    for command in ["OPTS UTF8 ON", "EPSV", "MLSD", "REST 0", "ABOR", "XYZZY"] {
        assert_eq!(
            session.send(command),
            "502 Command not found\r\n",
            "{command}"
        );
    }
}

#[test]
fn cwd_replies_follow_aurora() {
    let console = aurora_console();
    console.add_dir("/Game/Data");
    console.add_file("/Game/Data/Content.db", &[0; 10]);
    let mut session = RawSession::logged_in(&console);

    assert_eq!(
        session.send("CWD /Game/Data"),
        "250 Current directory is /Game/Data/\r\n"
    );
    assert_eq!(session.send("CDUP"), "250 Current directory is /Game/\r\n");
    assert_eq!(session.send("CWD /"), "250 Current directory is /\r\n");
    assert_eq!(session.send("CDUP"), "250 Current directory is /\r\n");
    assert_eq!(
        session.send("CWD /Game/Data/Content.db"),
        "550 Not a directory\r\n"
    );
    assert_eq!(session.send("CWD /Nowhere"), "550 No such directory\r\n");
}

#[test]
fn list_uses_aurora_lines_and_ignores_its_argument() {
    let console = aurora_console();
    console.add_dir("/Game/Data/GameData/4D5307E6_00000012");
    console.add_file("/Game/Data/Content.db", &[0; 1234567]);
    console.set_modified(
        "/Game/Data/Content.db",
        FileDate {
            year: 2026,
            month: 11,
            day: 3,
            hour: 14,
            minute: 5,
        },
    );
    console.add_file("/Game/Data/old.bin", &[0; 5]);
    console.set_modified(
        "/Game/Data/old.bin",
        FileDate {
            year: 2019,
            month: 1,
            day: 5,
            hour: 9,
            minute: 0,
        },
    );
    let mut session = RawSession::logged_in(&console);
    session.send("CWD /Game/Data");

    let (preliminary, listing, completion) = session.download("LIST /somewhere/else");

    assert_eq!(preliminary, "150 Opening connection\r\n");
    assert_eq!(completion, "226 Transfer complete\r\n");
    assert_eq!(
        listing,
        "-rw-rw-rw-   1 root root       1234567 Nov 03 14:05 Content.db\r\n\
         drwxrwxrwx   1 root root             0 Nov 07 12:00 GameData\r\n\
         -rw-rw-rw-   1 root root             5 Jan 05 2019 old.bin\r\n"
    );

    let (_, nlst, _) = session.download("NLST -la");
    assert_eq!(nlst, listing);
}

#[test]
fn listing_the_root_shows_drive_lines() {
    let console = aurora_console();
    console.add_dir("/Game/Data");
    console.add_dir("/Hdd1");
    let mut session = RawSession::logged_in(&console);

    let (_, listing, completion) = session.download("LIST");

    assert_eq!(completion, "226 Transfer complete\r\n");
    assert_eq!(
        listing,
        "drwxrwxrwx   1 root root             0 Jan 01  2000 Game\r\n\
         drwxrwxrwx   1 root root             0 Jan 01  2000 Hdd1\r\n"
    );
}

#[test]
fn transfers_answer_with_aurora_texts_and_pasv_keeps_its_port() {
    let console = aurora_console();
    console.add_dir("/Game/Data");
    let mut session = RawSession::logged_in(&console);
    session.send("CWD /Game/Data");

    let first_port = session.passive_port();
    let second_port = session.passive_port();
    assert_eq!(first_port, second_port);

    let mut data = TcpStream::connect(("127.0.0.1", first_port)).expect("data connection");
    assert_eq!(session.send("STOR a.bin"), "150 Opening connection\r\n");
    data.write_all(&[1, 2, 3]).expect("uploading");
    drop(data);
    assert_eq!(session.read_line(), "226 File transferred\r\n");
    assert_eq!(console.file("/Game/Data/a.bin"), Some(vec![1, 2, 3]));

    let (preliminary, _, completion) = session.download("RETR a.bin");
    assert_eq!(preliminary, "150 Waiting for connection\r\n");
    assert_eq!(completion, "226 File transferred\r\n");
    assert_eq!(session.send("SIZE a.bin"), "213 3\r\n");
    assert_eq!(session.send("SIZE nope.bin"), "550 File not found\r\n");
    assert_eq!(session.send("SIZE /Game"), "550 Not a file\r\n");
    assert_eq!(
        session.send("RETR nope.bin"),
        "550 Could not open file.  Error:  2\r\n"
    );
}
