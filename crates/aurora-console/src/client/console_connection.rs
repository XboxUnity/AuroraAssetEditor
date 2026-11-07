use std::io::{self, Cursor, Read};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};

use suppaftp::types::{FileType, Mode, Response};
use suppaftp::{FtpError, FtpStream, Status};

use super::{AuroraRevision, ConsoleError, ConsoleStatus, Timeouts};
use crate::ftp::{Features, FtpReply, features_from_reply, terminated_lines};
use crate::settings::FtpSettings;

/// One logged-in control connection, checked to be Aurora.
pub(super) struct ConsoleConnection {
    stream: FtpStream,
    features: Features,
}

impl ConsoleConnection {
    /// Resolves, connects, logs in and runs the Aurora check, reporting each stage.
    pub(super) fn open(
        settings: &FtpSettings,
        timeouts: Timeouts,
        on_status: &mut dyn FnMut(&ConsoleStatus),
    ) -> Result<(ConsoleConnection, AuroraRevision), ConsoleError> {
        timeouts.check()?;
        let host = &settings.host;
        on_status(&ConsoleStatus::Connecting { host: host.clone() });
        let port = settings.port_number()?;
        let socket = connect_socket(host, port, timeouts)?;

        let stream = FtpStream::connect_with_stream(socket)
            .map_err(protocol_error("read the greeting"))?
            .passive_stream_builder(move |address| open_data_socket(address, timeouts));
        let mut connection = ConsoleConnection {
            stream,
            features: Features::default(),
        };
        connection.stream.set_mode(Mode::Passive);
        connection.log_in(settings)?;

        on_status(&ConsoleStatus::CheckingForAurora { host: host.clone() });
        connection.read_features()?;
        let revision = connection.check_for_aurora()?;
        // Without binary mode suppaftp converts LF to CRLF on upload.
        connection
            .stream
            .transfer_type(FileType::Binary)
            .map_err(protocol_error("switch to binary mode"))?;

        on_status(&ConsoleStatus::AuroraFound {
            revision: revision.clone(),
        });
        Ok((connection, revision))
    }

    /// Sends one command and returns the console's reply whatever its code.
    pub(super) fn execute(&mut self, command: &str) -> Result<FtpReply, ConsoleError> {
        let action = || format!("send {command}");
        // With no expected codes every reply arrives as `UnexpectedResponse`, so both arms are replies.
        match self.stream.custom_command(command, &[]) {
            Ok(response) | Err(FtpError::UnexpectedResponse(response)) => {
                reply_from(response, &action)
            }
            Err(source) => Err(ConsoleError::Protocol {
                action: action(),
                source,
            }),
        }
    }

    pub(super) fn features(&self) -> Features {
        self.features
    }

    pub(super) fn list(&mut self, path: &str, limit: u64) -> Result<Vec<String>, ConsoleError> {
        self.read_listing("LIST", path, limit)
    }

    pub(super) fn mlsd(&mut self, path: &str, limit: u64) -> Result<Vec<String>, ConsoleError> {
        self.read_listing("MLSD", path, limit)
    }

    /// Reads the file, which the listing gave as `listed` bytes, and fails on any other length.
    pub(super) fn retrieve(&mut self, name: &str, listed: u64) -> Result<Vec<u8>, ConsoleError> {
        let action = format!("download {name}");
        let mut transfer = self
            .stream
            .retr_as_stream(name)
            .map_err(protocol_error(action.clone()))?;
        // One byte past the listed size is enough to tell that the console sends more.
        let bytes = self.read_data(&mut transfer, listed.saturating_add(1), &action)?;
        let received = bytes.len() as u64;
        if received != listed {
            self.abandon_control_connection();
            return Err(ConsoleError::SizeMismatch {
                file: name.to_owned(),
                listed,
                received,
            });
        }
        transfer.finish().map_err(protocol_error(action))?;
        Ok(bytes)
    }

    /// Stores the bytes unchanged in the working folder, replacing any file of that name.
    pub(super) fn store(&mut self, name: &str, bytes: &[u8]) -> Result<(), ConsoleError> {
        self.stream
            .put_file(name, &mut Cursor::new(bytes))
            .map(|_| ())
            .map_err(protocol_error(format!("upload {name}")))
    }

    /// Lines are split like the old `ReadLine`. A listing longer than `limit` bytes is an error.
    fn read_listing(
        &mut self,
        verb: &str,
        path: &str,
        limit: u64,
    ) -> Result<Vec<String>, ConsoleError> {
        let action = format!("list {path}");
        let (_, mut transfer) = self
            .stream
            .custom_data_command(
                format!("{verb} {path}"),
                &[Status::AboutToSend, Status::AlreadyOpen],
            )
            .map_err(protocol_error(action.clone()))?;
        let bytes = self.read_data(&mut transfer, limit.saturating_add(1), &action)?;
        if bytes.len() as u64 > limit {
            self.abandon_control_connection();
            return Err(ConsoleError::ListingTooLarge {
                path: path.to_owned(),
                limit,
            });
        }
        transfer.finish().map_err(protocol_error(action))?;
        Ok(terminated_lines(&bytes).collect())
    }

    /// A failed read closes the control connection before the transfer drops: dropping a transfer
    /// waits for the console's completion reply, which a stalled console never sends.
    fn read_data(
        &self,
        transfer: &mut impl Read,
        limit: u64,
        action: &str,
    ) -> Result<Vec<u8>, ConsoleError> {
        let mut bytes = Vec::new();
        transfer
            .by_ref()
            .take(limit)
            .read_to_end(&mut bytes)
            .map_err(|source| {
                self.abandon_control_connection();
                ConsoleError::Protocol {
                    action: action.to_owned(),
                    source: FtpError::ConnectionError(source),
                }
            })?;
        Ok(bytes)
    }

    /// Only for a connection that is about to be discarded.
    fn abandon_control_connection(&self) {
        if let Err(error) = self.stream.get_ref().shutdown(Shutdown::Both)
            && error.kind() != io::ErrorKind::NotConnected
        {
            log::warn!("could not close the control connection: {error}");
        }
    }

    /// Any refusal of USER or PASS, Aurora's 332 for an unknown user included, is a rejected login.
    fn log_in(&mut self, settings: &FtpSettings) -> Result<(), ConsoleError> {
        match self.stream.login(&settings.username, &settings.password) {
            Ok(()) => Ok(()),
            Err(FtpError::UnexpectedResponse(response)) => Err(ConsoleError::LoginRejected {
                code: response.status.code(),
            }),
            Err(source) => Err(ConsoleError::Protocol {
                action: "log in".to_owned(),
                source,
            }),
        }
    }

    fn read_features(&mut self) -> Result<(), ConsoleError> {
        let reply = self.execute("FEAT")?;
        if reply.is_success() {
            self.features = features_from_reply(&reply);
        }
        if self.features.utf8 {
            // The old client ignored the answer too.
            self.execute("OPTS UTF8 ON")?;
        }
        Ok(())
    }

    fn check_for_aurora(&mut self) -> Result<AuroraRevision, ConsoleError> {
        let reply = self.execute("SITE REVISION")?;
        if reply.is_success() {
            Ok(AuroraRevision::new(reply.message))
        } else {
            Err(ConsoleError::NotAurora {
                reply: format!("{} {}", reply.code, reply.message),
            })
        }
    }
}

fn reply_from(response: Response, action: &dyn Fn() -> String) -> Result<FtpReply, ConsoleError> {
    FtpReply::from_body(&response.body).ok_or_else(|| ConsoleError::Protocol {
        action: action(),
        source: FtpError::BadResponse,
    })
}

fn protocol_error(action: impl Into<String>) -> impl FnOnce(FtpError) -> ConsoleError {
    move |source| ConsoleError::Protocol {
        action: action.into(),
        source,
    }
}

fn connect_socket(host: &str, port: u16, timeouts: Timeouts) -> Result<TcpStream, ConsoleError> {
    let resolve_error = |source| ConsoleError::Resolve {
        host: host.to_owned(),
        source,
    };
    let mut addresses = (host, port).to_socket_addrs().map_err(resolve_error)?;
    let first = addresses.next().ok_or_else(|| {
        resolve_error(io::Error::new(
            io::ErrorKind::NotFound,
            "the name has no address",
        ))
    })?;

    let mut attempt = TcpStream::connect_timeout(&first, timeouts.connect);
    for address in addresses {
        if attempt.is_ok() {
            break;
        }
        attempt = TcpStream::connect_timeout(&address, timeouts.connect);
    }
    attempt
        .and_then(|socket| apply_read_write_timeouts(socket, timeouts))
        .map_err(|source| ConsoleError::Connect {
            host: host.to_owned(),
            port,
            source,
        })
}

fn open_data_socket(address: SocketAddr, timeouts: Timeouts) -> Result<TcpStream, FtpError> {
    TcpStream::connect_timeout(&address, timeouts.connect)
        .and_then(|socket| apply_read_write_timeouts(socket, timeouts))
        .map_err(FtpError::ConnectionError)
}

fn apply_read_write_timeouts(socket: TcpStream, timeouts: Timeouts) -> io::Result<TcpStream> {
    socket.set_read_timeout(Some(timeouts.read))?;
    socket.set_write_timeout(Some(timeouts.read))?;
    Ok(socket)
}
