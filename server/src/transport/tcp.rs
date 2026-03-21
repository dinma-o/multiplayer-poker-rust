use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use common::wire::{ClientRequest, ErrorCode, ErrorPayload, ServerResponse};

use crate::app::ServerApp;

pub const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";

pub type SharedServerApp = Arc<Mutex<ServerApp>>;

pub fn serve(app: SharedServerApp) -> io::Result<()> {
    serve_on(DEFAULT_BIND_ADDR, app)
}

pub fn serve_on(bind_addr: &str, app: SharedServerApp) -> io::Result<()> {
    let listener = TcpListener::bind(bind_addr)?;
    println!("PokerDUD TCP server listening on {bind_addr}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer = stream.peer_addr().ok();
                let app = Arc::clone(&app);
                thread::spawn(move || {
                    if let Err(error) = handle_client(stream, app) {
                        match peer {
                            Some(peer_addr) => {
                                eprintln!("tcp client {peer_addr} disconnected with error: {error}")
                            }
                            None => eprintln!("tcp client disconnected with error: {error}"),
                        }
                    }
                });
            }
            Err(error) => {
                eprintln!("failed to accept tcp client: {error}");
            }
        }
    }

    Ok(())
}

fn handle_client(stream: TcpStream, app: SharedServerApp) -> io::Result<()> {
    let read_stream = stream.try_clone()?;
    let mut reader = BufReader::new(read_stream);
    let mut writer = stream;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Ok(());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = handle_request_line(trimmed, &app);
        write_response(&mut writer, &response)?;
    }
}

fn handle_request_line(line: &str, app: &SharedServerApp) -> ServerResponse {
    let request: ClientRequest = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(error) => {
            return ServerResponse::error(
                "invalid-request",
                ErrorPayload::new(
                    ErrorCode::InvalidRequest,
                    format!("invalid request JSON: {error}"),
                ),
            );
        }
    };

    match app.lock() {
        Ok(mut app) => app.handle_request(request),
        Err(_) => ServerResponse::error(
            request.request_id,
            ErrorPayload::new(ErrorCode::Internal, "server application lock poisoned"),
        ),
    }
}

fn write_response(stream: &mut TcpStream, response: &ServerResponse) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, response).map_err(io::Error::other)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account_store::AccountStore;
    use common::wire::ResponsePayload;
    use tempfile::{TempDir, tempdir};

    struct TestFixture {
        _tmp: TempDir,
        app: SharedServerApp,
    }

    fn test_fixture() -> TestFixture {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("accounts.json");
        let store = AccountStore::load(path).expect("load account store");
        TestFixture {
            _tmp: dir,
            app: Arc::new(Mutex::new(ServerApp::new(store))),
        }
    }

    #[test]
    fn malformed_json_returns_invalid_request_error() {
        let fixture = test_fixture();
        let response = handle_request_line("{not-json", &fixture.app);
        assert!(!response.ok);
        let error = response.error.expect("error payload");
        assert_eq!(error.code, ErrorCode::InvalidRequest);
    }

    #[test]
    fn valid_json_routes_to_server_app() {
        let fixture = test_fixture();
        let response = handle_request_line(
            r#"{"request_id":"req-1","session_id":null,"op":{"type":"list_tables"}}"#,
            &fixture.app,
        );

        assert!(response.ok);
        match response.payload.expect("payload") {
            ResponsePayload::Tables(tables) => {
                assert_eq!(tables.tables.len(), 1);
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[test]
    fn response_writer_uses_newline_delimited_json() {
        let mut bytes = Vec::new();
        let response = ServerResponse::ok(
            "req-2",
            ResponsePayload::Tables(
                crate::app::ServerApp::new(
                    AccountStore::load(tempdir().expect("tempdir").path().join("accounts.json"))
                        .expect("load account store"),
                )
                .list_tables(),
            ),
        );

        serde_json::to_writer(&mut bytes, &response).expect("serialize response");
        bytes.extend_from_slice(b"\n");

        assert_eq!(bytes.last().copied(), Some(b'\n'));
    }
}
