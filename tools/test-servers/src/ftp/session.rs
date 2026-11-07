use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use super::ListingStyle;
use super::console_state::{Shared, shut_down};
use super::file_system::{path_key, resolve_path};
use super::listing_line::listing_line;

const POLL_INTERVAL: Duration = Duration::from_millis(5);
const DATA_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MASKED_PASSWORD: &str = "PASS ****";
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
    pending_user: Option<String>,
    logged_in: bool,
    passive_listener: Option<TcpListener>,
}

impl<'a> Session<'a> {
    fn new(shared: &'a Shared, stream: TcpStream) -> Session<'a> {
        Session {
            shared,
            control: BufReader::new(stream),
            current_folder: Vec::new(),
            pending_user: None,
            logged_in: false,
            passive_listener: None,
        }
    }

    fn run(&mut self) -> io::Result<()> {
        self.control
            .get_ref()
            .set_read_timeout(Some(POLL_INTERVAL))?;
        self.reply("220 Fake Xbox 360 FTP ready")?;
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
        let (verb, argument) = line.split_once(' ').unwrap_or((line, ""));
        let verb = verb.to_uppercase();
        self.log_command(&verb, line);
        if self
            .shared
            .timing_out_next_command
            .swap(false, Ordering::SeqCst)
        {
            self.reply("421 Timeout")?;
            shut_down(self.control.get_ref());
            return Ok(Flow::Close);
        }
        if !self.logged_in && !matches!(verb.as_str(), "USER" | "PASS" | "QUIT") {
            self.reply("530 Please log in")?;
            return Ok(Flow::Continue);
        }
        match verb.as_str() {
            "USER" => self.user(argument)?,
            "PASS" => self.pass(argument)?,
            "FEAT" => self.feat()?,
            "OPTS" => self.opts(argument)?,
            "SITE" => self.site(argument)?,
            "NOOP" | "TYPE" => self.reply("200 OK")?,
            "PWD" => self.pwd()?,
            "CWD" => self.cwd(argument)?,
            "PASV" => self.pasv()?,
            "LIST" => self.list(argument)?,
            "MLSD" => self.mlsd(argument)?,
            "RETR" => self.retr(argument)?,
            "STOR" => self.stor(argument)?,
            "SIZE" => self.size(argument)?,
            "QUIT" => {
                self.reply("221 Goodbye")?;
                return Ok(Flow::Close);
            }
            _ => self.reply("500 Unknown command")?,
        }
        Ok(Flow::Continue)
    }

    fn log_command(&self, verb: &str, line: &str) {
        let entry = if verb == "PASS" {
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
        self.pending_user = Some(name.to_owned());
        self.reply("331 Password required")
    }

    fn pass(&mut self, password: &str) -> io::Result<()> {
        let config = &self.shared.config;
        let accepted = self.pending_user.as_deref() == Some(config.username.as_str())
            && password == config.password;
        if accepted {
            self.logged_in = true;
            self.shared.state().login_count += 1;
            self.reply("230 Logged in")
        } else {
            self.reply("530 Login incorrect")
        }
    }

    fn feat(&mut self) -> io::Result<()> {
        let Some(info_lines) = self.shared.config.features.info_lines() else {
            return self.reply("500 Unknown command");
        };
        let mut body = String::from("211-Features:\r\n");
        for info_line in info_lines {
            body.push_str(info_line);
            body.push_str("\r\n");
        }
        body.push_str("211 End\r\n");
        self.write_raw(&body)
    }

    fn opts(&mut self, argument: &str) -> io::Result<()> {
        if argument.eq_ignore_ascii_case("UTF8 ON") {
            self.reply("200 UTF8 is on")
        } else {
            self.reply("501 Option not supported")
        }
    }

    fn site(&mut self, argument: &str) -> io::Result<()> {
        if !argument.eq_ignore_ascii_case("REVISION") {
            return self.reply("502 Command not implemented");
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
        let target = resolve_path(&self.current_folder, argument);
        if self.shared.state().file_system.is_folder(&target) {
            self.current_folder = target;
            self.reply("250 Folder changed")
        } else {
            self.reply("550 No such folder")
        }
    }

    fn pasv(&mut self) -> io::Result<()> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        self.passive_listener = Some(listener);
        let (high, low) = (port / 256, port % 256);
        self.reply(&format!(
            "227 Entering Passive Mode (127,0,0,1,{high},{low})"
        ))
    }

    fn list(&mut self, argument: &str) -> io::Result<()> {
        let style = match self.shared.config.listing_style {
            ListingStyle::Mlsd => ListingStyle::Unix,
            style => style,
        };
        self.send_listing(style, argument)
    }

    fn mlsd(&mut self, argument: &str) -> io::Result<()> {
        let config = &self.shared.config;
        if config.listing_style == ListingStyle::Mlsd || config.features.advertises_mlsd() {
            self.send_listing(ListingStyle::Mlsd, argument)
        } else {
            self.reply("500 Unknown command")
        }
    }

    fn send_listing(&mut self, style: ListingStyle, argument: &str) -> io::Result<()> {
        let path_argument = if argument.starts_with('-') {
            ""
        } else {
            argument
        };
        let folder = resolve_path(&self.current_folder, path_argument);
        let lines = {
            let state = self.shared.state();
            if !state.file_system.is_folder(&folder) {
                drop(state);
                return self.reply("550 No such folder");
            }
            state
                .file_system
                .entries_in(&folder)
                .iter()
                .map(|entry| {
                    let size = state
                        .listed_sizes
                        .get(&entry.key)
                        .copied()
                        .unwrap_or(entry.size);
                    listing_line(style, entry, size)
                })
                .collect::<Vec<_>>()
        };
        let Some(mut data) = self.begin_transfer()? else {
            return Ok(());
        };
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
            return self.reply("550 No such file");
        };
        let Some(mut data) = self.begin_transfer()? else {
            return Ok(());
        };
        if let Some(after) = stall_after {
            data.write_all(&bytes[..after.min(bytes.len())])?;
            return self.stay_silent_until_client_gives_up(&mut data);
        }
        let sent = keep.map_or(bytes.len(), |keep| keep.min(bytes.len()));
        data.write_all(&bytes[..sent])?;
        shut_down(&data);
        self.reply("226 Transfer complete")
    }

    fn stor(&mut self, argument: &str) -> io::Result<()> {
        let path = resolve_path(&self.current_folder, argument);
        let parent_exists = path
            .split_last()
            .is_some_and(|(_, parent)| self.shared.state().file_system.is_folder(parent));
        if !parent_exists {
            return self.reply("550 No such folder");
        }
        let Some(mut data) = self.begin_transfer()? else {
            return Ok(());
        };
        let bytes = self.read_upload(&mut data)?;
        self.shared.state().file_system.put_file(&path, &bytes);
        self.reply("226 Transfer complete")
    }

    fn size(&mut self, argument: &str) -> io::Result<()> {
        let path = resolve_path(&self.current_folder, argument);
        let length = self.shared.state().file_system.file(&path).map(<[u8]>::len);
        match length {
            Some(length) => self.reply(&format!("213 {length}")),
            None => self.reply("550 No such file"),
        }
    }

    /// Answers `150` and accepts the client's connection to the port `PASV` announced.
    fn begin_transfer(&mut self) -> io::Result<Option<TcpStream>> {
        let Some(listener) = self.passive_listener.take() else {
            self.reply("425 Use PASV first")?;
            return Ok(None);
        };
        self.reply("150 Opening data connection")?;
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

fn is_read_timeout(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}
