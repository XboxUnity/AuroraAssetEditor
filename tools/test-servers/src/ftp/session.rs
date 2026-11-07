use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use super::ListingStyle;
use super::console_state::{Shared, shut_down};
use super::fake_console_config::{FAKE_INPUT_HASH, FAKE_STORED_HASH};
use super::file_system::{path_key, resolve_path};
use super::listing_line::listing_line;

const POLL_INTERVAL: Duration = Duration::from_millis(5);
const DATA_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MASKED_PASSWORD: &str = "PASS ****";
/// Aurora's command table in its order. A typed verb runs the first entry it is a prefix of.
const COMMAND_TABLE: [&str; 30] = [
    "CDUP", "CWD", "DELE", "FEAT", "LIST", "MKD", "MKTS", "MDTM", "NLST", "NOOP", "PASS", "PASV",
    "PORT", "PWD", "QUIT", "RETR", "RMD", "RNFR", "RNTO", "SIZE", "STAT", "APPE", "STOR", "SYST",
    "TYPE", "USER", "XMKD", "SITE", "HELP", "XCRC",
];
const LOGIN_NOT_REQUIRED: [&str; 3] = ["PASS", "QUIT", "USER"];
/// Verbs outside Aurora's table that the non-Aurora variants answer.
const EXTENSION_VERBS: [&str; 2] = ["OPTS", "MLSD"];
const CLIENT_GONE: [io::ErrorKind; 5] = [
    io::ErrorKind::BrokenPipe,
    io::ErrorKind::ConnectionReset,
    io::ErrorKind::ConnectionAborted,
    io::ErrorKind::NotConnected,
    io::ErrorKind::UnexpectedEof,
];

enum Flow {
    Continue,
    Close,
}

/// Serves one control connection until the client quits or the connection closes.
pub(crate) fn serve(shared: &Shared, stream: TcpStream) {
    let outcome = Session::new(shared, stream).run();
    if let Err(error) = outcome
        && !shared.stopping.load(Ordering::SeqCst)
        && !CLIENT_GONE.contains(&error.kind())
    {
        eprintln!("fake console: a session failed: {error}");
    }
}

struct Session<'a> {
    shared: &'a Shared,
    control: BufReader<TcpStream>,
    current_folder: Vec<String>,
    user_accepted: bool,
    logged_in: bool,
    passive_listener: Option<TcpListener>,
}

impl<'a> Session<'a> {
    fn new(shared: &'a Shared, stream: TcpStream) -> Session<'a> {
        Session {
            shared,
            control: BufReader::new(stream),
            current_folder: Vec::new(),
            user_accepted: false,
            logged_in: false,
            passive_listener: None,
        }
    }

    fn run(&mut self) -> io::Result<()> {
        self.control
            .get_ref()
            .set_read_timeout(Some(POLL_INTERVAL))?;
        self.reply("220 FtpDll Ready")?;
        let mut line = String::new();
        loop {
            match self.control.read_line(&mut line) {
                Ok(0) => return Ok(()),
                Ok(_) => {
                    let flow = self.handle(line.trim_end_matches(['\r', '\n']))?;
                    line.clear();
                    if let Flow::Close = flow {
                        return Ok(());
                    }
                }
                // A blocked read is not woken by `shutdown` on Windows, so the loop polls the stop flag.
                Err(error) if is_read_timeout(&error) => {
                    if self.shared.stopping.load(Ordering::SeqCst) {
                        return Ok(());
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn handle(&mut self, line: &str) -> io::Result<Flow> {
        let (typed, argument) = line.split_once(' ').unwrap_or((line, ""));
        let verb = resolve_verb(typed);
        self.log_command(verb, line);
        if self
            .shared
            .timing_out_next_command
            .swap(false, Ordering::SeqCst)
        {
            self.reply("421 Timeout")?;
            shut_down(self.control.get_ref());
            return Ok(Flow::Close);
        }
        if line.is_empty() {
            self.reply("500 ?")?;
            return Ok(Flow::Continue);
        }
        if !self.logged_in && !verb.is_some_and(|verb| LOGIN_NOT_REQUIRED.contains(&verb)) {
            self.reply("530 Not logged in")?;
            return Ok(Flow::Continue);
        }
        match verb {
            Some("USER") => self.user(argument)?,
            Some("PASS") => self.pass(argument)?,
            Some("FEAT") => self.feat()?,
            Some("OPTS") => self.opts(argument)?,
            Some("SITE") => self.site(argument)?,
            Some("NOOP") => self.reply("200 ori r0, r0, 0")?,
            Some("TYPE") => self.reply(&format!("200 Type set to {argument}"))?,
            Some("PWD") => self.pwd()?,
            Some("CWD") => self.cwd(argument)?,
            Some("CDUP") => self.cdup()?,
            Some("PASV") => self.pasv()?,
            Some("LIST" | "NLST") => self.list()?,
            Some("MLSD") => self.mlsd(argument)?,
            Some("RETR") => self.retr(argument)?,
            Some("STOR") => self.stor(argument)?,
            Some("SIZE") => self.size(argument)?,
            Some("QUIT") => {
                self.reply("221 Goodbye")?;
                return Ok(Flow::Close);
            }
            _ => self.reply("502 Command not found")?,
        }
        Ok(Flow::Continue)
    }

    fn log_command(&self, verb: Option<&str>, line: &str) {
        let entry = if verb == Some("PASS") {
            MASKED_PASSWORD
        } else {
            line
        };
        self.shared.state().commands.push(entry.to_owned());
    }

    fn reply(&mut self, text: &str) -> io::Result<()> {
        self.write_raw(&format!("{text}\r\n"))
    }

    fn write_raw(&mut self, text: &str) -> io::Result<()> {
        self.control.get_mut().write_all(text.as_bytes())
    }

    fn user(&mut self, name: &str) -> io::Result<()> {
        if self.logged_in {
            return self.reply("530 Already logged in");
        }
        self.user_accepted = name.eq_ignore_ascii_case(&self.shared.config.username);
        if self.user_accepted {
            self.reply(&format!("331 User {name} OK, need password"))
        } else {
            self.reply("332 User not OK")
        }
    }

    fn pass(&mut self, password: &str) -> io::Result<()> {
        if self.logged_in {
            return self.reply("530 Already logged in");
        }
        if !std::mem::take(&mut self.user_accepted) {
            return self.reply("530 Need username");
        }
        if password == self.shared.config.password {
            self.logged_in = true;
            self.shared.state().login_count += 1;
            self.reply("230 User logged in")
        } else {
            self.reply(&format!(
                "530 Login failed: input = {FAKE_INPUT_HASH}, pass = {FAKE_STORED_HASH}"
            ))
        }
    }

    fn feat(&mut self) -> io::Result<()> {
        let Some(info_lines) = self.shared.config.features.info_lines() else {
            return self.reply("500 Unknown command");
        };
        let mut body = String::from("211-Extensions supported:\r\n");
        for info_line in info_lines {
            body.push_str(info_line);
            body.push_str("\r\n");
        }
        body.push_str("211 END\r\n");
        self.write_raw(&body)
    }

    fn opts(&mut self, argument: &str) -> io::Result<()> {
        if !self.shared.config.features.accepts_utf8_option() {
            return self.reply("502 Command not found");
        }
        if argument.eq_ignore_ascii_case("UTF8 ON") {
            self.reply("200 UTF8 is on")
        } else {
            self.reply("501 Option not supported")
        }
    }

    fn site(&mut self, argument: &str) -> io::Result<()> {
        if !argument.eq_ignore_ascii_case("REVISION") {
            return self.reply("502 Command SITE not implemented for this argument.");
        }
        let reply = self
            .shared
            .config
            .site_revision_reply
            .clone()
            .unwrap_or_else(|| "500 Unknown command".to_owned());
        self.reply(&reply)
    }

    fn pwd(&mut self) -> io::Result<()> {
        let mut shown = self
            .shared
            .state()
            .file_system
            .display_path(&self.current_folder);
        if !shown.ends_with('/') {
            shown.push('/');
        }
        self.reply(&format!("257 \"{shown}\""))
    }

    fn cwd(&mut self, argument: &str) -> io::Result<()> {
        let forced_reply = self.shared.state().folder_change_reply.clone();
        if let Some(reply) = forced_reply {
            return self.reply(&reply);
        }
        // Aurora hands `.` and `..` to the console's file system, which has no such folders.
        if argument
            .split('/')
            .any(|segment| matches!(segment, "." | ".."))
        {
            return self.reply("550 No such directory");
        }
        let target = resolve_path(&self.current_folder, argument);
        self.change_folder(target)
    }

    fn cdup(&mut self) -> io::Result<()> {
        let mut parent = self.current_folder.clone();
        parent.pop();
        self.change_folder(parent)
    }

    fn change_folder(&mut self, target: Vec<String>) -> io::Result<()> {
        let (is_folder, is_file) = {
            let state = self.shared.state();
            (
                state.file_system.is_folder(&target),
                state.file_system.file(&target).is_some(),
            )
        };
        if is_folder {
            let reply = format!("250 Current directory is {}", folder_text(&target));
            self.current_folder = target;
            self.reply(&reply)
        } else if is_file {
            self.reply("550 Not a directory")
        } else {
            self.reply("550 No such directory")
        }
    }

    /// Like Aurora's `PASV`, the session keeps one listener, so every `PASV` names the same port.
    fn pasv(&mut self) -> io::Result<()> {
        let port = match &self.passive_listener {
            Some(listener) => listener.local_addr()?.port(),
            None => {
                let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
                let port = listener.local_addr()?.port();
                self.passive_listener = Some(listener);
                port
            }
        };
        let (high, low) = (port / 256, port % 256);
        self.reply(&format!(
            "227 Entering Passive Mode (127,0,0,1,{high},{low})"
        ))
    }

    /// `LIST` and `NLST` list the working directory whatever argument they get, like Aurora.
    fn list(&mut self) -> io::Result<()> {
        let style = match self.shared.config.listing_style {
            ListingStyle::Mlsd => ListingStyle::Unix,
            style => style,
        };
        self.send_listing(style, "")
    }

    fn mlsd(&mut self, argument: &str) -> io::Result<()> {
        let config = &self.shared.config;
        if config.listing_style == ListingStyle::Mlsd || config.features.advertises_mlsd() {
            self.send_listing(ListingStyle::Mlsd, argument)
        } else {
            self.reply("502 Command not found")
        }
    }

    fn send_listing(&mut self, style: ListingStyle, argument: &str) -> io::Result<()> {
        let path_argument = if argument.starts_with('-') {
            ""
        } else {
            argument
        };
        let folder = resolve_path(&self.current_folder, path_argument);
        let stalled = self
            .shared
            .state()
            .stalled_listings
            .contains(&path_key(&folder));
        let lines = {
            let state = self.shared.state();
            if !state.file_system.is_folder(&folder) {
                drop(state);
                return self.reply("550 No such directory");
            }
            let at_root = folder.is_empty();
            state
                .file_system
                .entries_in(&folder)
                .iter()
                .filter(|entry| !(at_root && !entry.is_folder))
                .map(|entry| {
                    let size = state
                        .listed_sizes
                        .get(&entry.key)
                        .copied()
                        .unwrap_or(entry.size);
                    listing_line(style, entry, size, at_root)
                })
                .collect::<Vec<_>>()
        };
        let Some(mut data) = self.begin_transfer("150 Opening connection")? else {
            return Ok(());
        };
        if stalled {
            return self.stay_silent_until_client_gives_up(&mut data);
        }
        for line in lines {
            data.write_all(format!("{line}\r\n").as_bytes())?;
        }
        shut_down(&data);
        self.reply("226 Transfer complete")
    }

    fn retr(&mut self, argument: &str) -> io::Result<()> {
        let path = resolve_path(&self.current_folder, argument);
        let key = path_key(&path);
        let (bytes, keep, stall_after) = {
            let state = self.shared.state();
            (
                state.file_system.file(&path).map(<[u8]>::to_vec),
                state.truncated_downloads.get(&key).copied(),
                state.stalled_downloads.get(&key).copied(),
            )
        };
        let Some(bytes) = bytes else {
            return self.reply("550 Could not open file.  Error:  2");
        };
        let Some(mut data) = self.begin_transfer("150 Waiting for connection")? else {
            return Ok(());
        };
        if let Some(after) = stall_after {
            data.write_all(&bytes[..after.min(bytes.len())])?;
            return self.stay_silent_until_client_gives_up(&mut data);
        }
        let sent = keep.map_or(bytes.len(), |keep| keep.min(bytes.len()));
        data.write_all(&bytes[..sent])?;
        shut_down(&data);
        self.reply("226 File transferred")
    }

    fn stor(&mut self, argument: &str) -> io::Result<()> {
        let path = resolve_path(&self.current_folder, argument);
        let parent_exists = path
            .split_last()
            .is_some_and(|(_, parent)| self.shared.state().file_system.is_folder(parent));
        if !parent_exists {
            return self.reply("550 Could not create file");
        }
        let Some(mut data) = self.begin_transfer("150 Opening connection")? else {
            return Ok(());
        };
        let bytes = self.read_upload(&mut data)?;
        self.shared.state().file_system.put_file(&path, &bytes);
        self.reply("226 File transferred")
    }

    fn size(&mut self, argument: &str) -> io::Result<()> {
        let path = resolve_path(&self.current_folder, argument);
        let (length, is_folder) = {
            let state = self.shared.state();
            (
                state.file_system.file(&path).map(<[u8]>::len),
                state.file_system.is_folder(&path),
            )
        };
        match length {
            Some(length) => self.reply(&format!("213 {length}")),
            None if is_folder => self.reply("550 Not a file"),
            None => self.reply("550 File not found"),
        }
    }

    /// Answers with `preliminary` and accepts the client's connection to the port `PASV` announced.
    fn begin_transfer(&mut self, preliminary: &str) -> io::Result<Option<TcpStream>> {
        let listener = self
            .passive_listener
            .as_ref()
            .map(TcpListener::try_clone)
            .transpose()?;
        let Some(listener) = listener else {
            self.reply("425 Use PASV first")?;
            return Ok(None);
        };
        self.reply(preliminary)?;
        listener.set_nonblocking(true)?;
        let deadline = Instant::now() + DATA_CONNECT_TIMEOUT;
        loop {
            match listener.accept() {
                Ok((data, _)) => {
                    data.set_nonblocking(false)?;
                    data.set_write_timeout(Some(DATA_CONNECT_TIMEOUT))?;
                    self.shared.state().data_sockets.push(data.try_clone()?);
                    return Ok(Some(data));
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.ensure_running(deadline)?;
                    thread::sleep(POLL_INTERVAL);
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn ensure_running(&self, deadline: Instant) -> io::Result<()> {
        if self.shared.stopping.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "the fake console is stopping",
            ));
        }
        if Instant::now() > deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the client never opened the data connection",
            ));
        }
        Ok(())
    }

    fn read_upload(&self, data: &mut TcpStream) -> io::Result<Vec<u8>> {
        data.set_read_timeout(Some(POLL_INTERVAL))?;
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match data.read(&mut chunk) {
                Ok(0) => return Ok(bytes),
                Ok(count) => bytes.extend_from_slice(&chunk[..count]),
                Err(error) if is_read_timeout(&error) => {
                    self.ensure_running(Instant::now() + DATA_CONNECT_TIMEOUT)?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Holds the data connection open without sending anything, like a console that hung.
    fn stay_silent_until_client_gives_up(&self, data: &mut TcpStream) -> io::Result<()> {
        data.set_read_timeout(Some(POLL_INTERVAL))?;
        let mut probe = [0u8; 1];
        while !self.shared.stopping.load(Ordering::SeqCst) {
            match data.read(&mut probe) {
                Ok(0) => break,
                Ok(_) => {}
                Err(error) if is_read_timeout(&error) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

/// The command a typed verb runs, matched case-insensitively as a prefix like Aurora does: `PAS`
/// runs `PASS`, and a verb longer than a table name never matches it.
fn resolve_verb(typed: &str) -> Option<&'static str> {
    if typed.is_empty() {
        return None;
    }
    let is_prefix_of = |name: &str| {
        name.as_bytes()
            .get(..typed.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(typed.as_bytes()))
    };
    COMMAND_TABLE
        .into_iter()
        .find(|name| is_prefix_of(name))
        .or_else(|| {
            EXTENSION_VERBS
                .into_iter()
                .find(|name| name.eq_ignore_ascii_case(typed))
        })
}

/// A folder as Aurora echoes it: absolute, with a trailing slash.
fn folder_text(segments: &[String]) -> String {
    if segments.is_empty() {
        return "/".to_owned();
    }
    format!("/{}/", segments.join("/"))
}

fn is_read_timeout(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}
