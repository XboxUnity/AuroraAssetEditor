use aurora_console::{
    AuroraRevision, ConsoleClient, ConsoleError, ConsoleStatus, FtpSettings, Timeouts,
};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use test_servers::{FakeConsole, FakeConsoleConfig, FakeFeatures, ListingStyle};

const HOST: &str = "127.0.0.1";
const SHORT: Duration = Duration::from_millis(500);

fn timeouts() -> Timeouts {
    Timeouts {
        connect: SHORT,
        read: SHORT,
    }
}

fn settings(port: u16, username: &str, password: &str) -> FtpSettings {
    FtpSettings {
        host: HOST.to_owned(),
        port: port.to_string(),
        username: username.to_owned(),
        password: password.to_owned(),
    }
}

fn client_for(console: &FakeConsole) -> ConsoleClient {
    ConsoleClient::new(
        settings(console.port(), "xboxftp", "xboxftp"),
        timeouts(),
        |_| {},
    )
}

fn console_with_site_reply(reply: Option<&str>) -> FakeConsole {
    FakeConsole::start(FakeConsoleConfig {
        site_revision_reply: reply.map(str::to_owned),
        ..FakeConsoleConfig::default()
    })
}

fn revision_of(console: &FakeConsole) -> Result<AuroraRevision, ConsoleError> {
    client_for(console).test_connection()
}

#[test]
fn test_connection_reports_aurora_revision() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let statuses = Arc::new(Mutex::new(Vec::<String>::new()));
    let mut client = ConsoleClient::new(
        settings(console.port(), "xboxftp", "xboxftp"),
        timeouts(),
        {
            let statuses = Arc::clone(&statuses);
            move |status: &ConsoleStatus| statuses.lock().unwrap().push(status.to_string())
        },
    );

    let revision = client.test_connection().unwrap();

    assert_eq!(revision.as_str(), "1655");
    assert_eq!(
        *statuses.lock().unwrap(),
        [
            "Connecting to 127.0.0.1...",
            "Connected to 127.0.0.1, checking for Aurora...",
            "Connected to Aurora revision 1655",
        ]
    );
    assert_eq!(
        console.commands()[..6],
        [
            "USER xboxftp",
            "PASS ****",
            "FEAT",
            "OPTS UTF8 ON",
            "SITE REVISION",
            "TYPE I"
        ]
    );
}

#[test]
fn multi_line_revision_uses_final_line() {
    let console = console_with_site_reply(Some("211-Aurora\r\n211 0.7b.2"));
    assert_eq!(revision_of(&console).unwrap().as_str(), "0.7b.2");
}

#[test]
fn unusual_success_code_counts() {
    let console = console_with_site_reply(Some("299 r1"));
    assert_eq!(revision_of(&console).unwrap().as_str(), "r1");
}

#[test]
fn refused_site_revision_is_not_aurora() {
    let console = console_with_site_reply(None);
    match revision_of(&console) {
        Err(ConsoleError::NotAurora { reply }) => assert!(reply.contains("500"), "{reply}"),
        other => panic!("expected NotAurora, got {other:?}"),
    }
}

#[test]
fn wrong_password_is_login_rejected() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut client = ConsoleClient::new(
        settings(console.port(), "xboxftp", "wrong"),
        timeouts(),
        |_| {},
    );
    match client.test_connection() {
        Err(ConsoleError::LoginRejected { reply }) => assert!(reply.contains("530"), "{reply}"),
        other => panic!("expected LoginRejected, got {other:?}"),
    }
}

#[test]
fn utf8_feature_sends_opts() {
    let console = FakeConsole::start(FakeConsoleConfig {
        features: FakeFeatures::MlsdAndUtf8,
        listing_style: ListingStyle::Unix,
        ..FakeConsoleConfig::default()
    });
    revision_of(&console).unwrap();
    let commands = console.commands();
    let position = |wanted: &str| commands.iter().position(|command| command == wanted);
    let opts = position("OPTS UTF8 ON").expect("OPTS UTF8 ON was not sent");
    let site = position("SITE REVISION").expect("SITE REVISION was not sent");
    assert!(opts < site, "{commands:?}");
}

#[test]
fn refused_feat_still_connects() {
    let console = FakeConsole::start(FakeConsoleConfig {
        features: FakeFeatures::Refused,
        ..FakeConsoleConfig::default()
    });
    assert_eq!(revision_of(&console).unwrap().as_str(), "1655");
    assert!(
        console
            .commands()
            .iter()
            .all(|command| !command.starts_with("OPTS")),
        "{:?}",
        console.commands()
    );
}

#[test]
fn closed_port_is_connect_error() {
    let listener = TcpListener::bind((HOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut client = ConsoleClient::new(settings(port, "xboxftp", "xboxftp"), timeouts(), |_| {});
    match client.test_connection() {
        Err(ConsoleError::Connect {
            port: reported_port,
            ..
        }) => assert_eq!(reported_port, port),
        other => panic!("expected Connect, got {other:?}"),
    }
}

#[test]
fn silent_server_times_out() {
    let listener = TcpListener::bind((HOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut client = ConsoleClient::new(settings(port, "xboxftp", "xboxftp"), timeouts(), |_| {});

    let started = Instant::now();
    let result = client.test_connection();

    assert!(
        matches!(&result, Err(ConsoleError::Protocol { action, .. }) if action == "read the greeting"),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    drop(listener);
}

#[test]
fn reuses_live_connection() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut client = client_for(&console);

    client.ensure_connected().unwrap();
    client.ensure_connected().unwrap();

    assert_eq!(console.login_count(), 1);
    assert!(console.commands().iter().any(|command| command == "NOOP"));
}

#[test]
fn reconnects_after_console_drops_idle_connection() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut client = client_for(&console);

    client.ensure_connected().unwrap();
    console.drop_connections();
    client.ensure_connected().unwrap();

    assert_eq!(console.login_count(), 2);
}

#[test]
fn failed_reconnect_returns_the_connect_error() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut client = client_for(&console);

    client.ensure_connected().unwrap();
    console.drop_connections();
    console.refuse_new_connections();

    match client.ensure_connected() {
        Err(ConsoleError::Connect { .. }) => {}
        other => panic!("expected Connect, got {other:?}"),
    }
}

#[test]
fn test_connection_always_reconnects() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut client = client_for(&console);

    client.test_connection().unwrap();
    client.test_connection().unwrap();

    assert_eq!(console.login_count(), 2);
}

#[test]
fn non_numeric_port_falls_back_to_21() {
    let settings = FtpSettings {
        host: HOST.to_owned(),
        port: "abc".to_owned(),
        ..FtpSettings::default()
    };
    assert_eq!(settings.port_number().unwrap(), 21);
    let mut client = ConsoleClient::new(settings, timeouts(), |_| {});
    match client.test_connection() {
        Err(ConsoleError::Connect { port, .. }) => assert_eq!(port, 21),
        other => panic!("expected Connect, got {other:?}"),
    }
}

#[test]
fn reconnects_after_console_sends_421_before_closing() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut client = client_for(&console);
    client.ensure_connected().unwrap();

    console.time_out_next_command();

    client.ensure_connected().unwrap();
    assert_eq!(console.login_count(), 2);
}

#[test]
fn not_aurora_drops_the_stream() {
    let console = console_with_site_reply(None);
    let mut client = client_for(&console);

    assert!(matches!(
        client.ensure_connected(),
        Err(ConsoleError::NotAurora { .. })
    ));
    assert!(matches!(
        client.ensure_connected(),
        Err(ConsoleError::NotAurora { .. })
    ));
    assert_eq!(console.login_count(), 2);
}

#[test]
fn login_rejected_drops_the_stream() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut client = ConsoleClient::new(
        settings(console.port(), "xboxftp", "wrong"),
        timeouts(),
        |_| {},
    );

    for _ in 0..2 {
        assert!(matches!(
            client.ensure_connected(),
            Err(ConsoleError::LoginRejected { .. })
        ));
    }
    assert_eq!(
        console
            .commands()
            .iter()
            .filter(|c| c.starts_with("USER"))
            .count(),
        2
    );
}

#[test]
fn zero_timeout_is_named_in_the_error() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    for (name, timeouts) in [
        (
            "connect",
            Timeouts {
                connect: Duration::ZERO,
                read: SHORT,
            },
        ),
        (
            "read",
            Timeouts {
                connect: SHORT,
                read: Duration::ZERO,
            },
        ),
    ] {
        let mut client = ConsoleClient::new(
            settings(console.port(), "xboxftp", "xboxftp"),
            timeouts,
            |_| {},
        );
        match client.test_connection() {
            Err(ConsoleError::InvalidTimeout { timeout }) => assert_eq!(timeout, name),
            other => panic!("expected InvalidTimeout, got {other:?}"),
        }
    }
}
