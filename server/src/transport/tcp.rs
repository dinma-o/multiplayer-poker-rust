use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use common::wire::{ClientRequest, ErrorCode, ErrorPayload, ServerResponse};

use crate::app::ServerApp;

pub const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";

pub type SharedServerApp = Arc<ServerApp>;

pub fn serve(app: SharedServerApp) -> io::Result<()> {
    serve_on(DEFAULT_BIND_ADDR, app)
}

pub fn serve_on(bind_addr: &str, app: SharedServerApp) -> io::Result<()> {
    let listener = TcpListener::bind(bind_addr)?;
    println!("PokerDUD TCP server listening on {bind_addr}");

    serve_listener(listener, app, None)
}

fn serve_listener(
    listener: TcpListener,
    app: SharedServerApp,
    connection_limit: Option<usize>,
) -> io::Result<()> {
    let mut handler_threads = Vec::new();
    let mut accepted_connections = 0usize;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer = stream.peer_addr().ok();
                let app = Arc::clone(&app);
                let handler = thread::spawn(move || {
                    if let Err(error) = handle_client(stream, app) {
                        match peer {
                            Some(peer_addr) => {
                                eprintln!("tcp client {peer_addr} disconnected with error: {error}")
                            }
                            None => eprintln!("tcp client disconnected with error: {error}"),
                        }
                    }
                });

                accepted_connections = accepted_connections.saturating_add(1);
                if connection_limit.is_some() {
                    handler_threads.push(handler);
                }

                if connection_limit.is_some_and(|limit| accepted_connections >= limit) {
                    break;
                }
            }
            Err(error) => {
                eprintln!("failed to accept tcp client: {error}");
            }
        }
    }

    for handler in handler_threads {
        handler.join().expect("tcp client handler thread panicked");
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

    app.handle_request(request)
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
    use common::protocol::{HandStep, PokerVariant};
    use common::wire::{ClientRequest, RequestOp, ResponsePayload, ServerResponse};
    use std::sync::Barrier;
    use std::time::Duration;
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
            app: Arc::new(ServerApp::new(store)),
        }
    }

    struct SocketTestServer {
        _tmp: TempDir,
        address: String,
        handle: thread::JoinHandle<io::Result<()>>,
    }

    impl SocketTestServer {
        fn join(self) {
            self.handle
                .join()
                .expect("server thread panicked")
                .expect("server thread failed");
        }
    }

    struct TestClient {
        reader: BufReader<TcpStream>,
        writer: TcpStream,
        next_request_id: u64,
        session_id: Option<String>,
    }

    impl TestClient {
        fn connect(address: &str) -> io::Result<Self> {
            let writer = TcpStream::connect(address)?;
            writer.set_read_timeout(Some(Duration::from_secs(2)))?;
            writer.set_write_timeout(Some(Duration::from_secs(2)))?;
            let reader = BufReader::new(writer.try_clone()?);

            Ok(Self {
                reader,
                writer,
                next_request_id: 1,
                session_id: None,
            })
        }

        fn send(&mut self, op: RequestOp) -> ServerResponse {
            let request = ClientRequest {
                request_id: format!("req-{}", self.next_request_id),
                session_id: self.session_id.clone(),
                op,
            };
            self.next_request_id = self.next_request_id.saturating_add(1);

            let json = serde_json::to_string(&request).expect("serialize request");
            self.writer
                .write_all(json.as_bytes())
                .expect("write request");
            self.writer.write_all(b"\n").expect("write newline");
            self.writer.flush().expect("flush request");

            let mut line = String::new();
            self.reader
                .read_line(&mut line)
                .expect("read response line");
            let response: ServerResponse =
                serde_json::from_str(line.trim()).expect("deserialize response");

            match &response.payload {
                Some(ResponsePayload::Login(login)) => {
                    self.session_id = Some(login.session_id.clone());
                }
                Some(ResponsePayload::Logout(_)) => {
                    self.session_id = None;
                }
                _ => {}
            }

            response
        }
    }

    fn spawn_socket_test_server(connection_limit: usize) -> SocketTestServer {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("accounts.json");
        let store = AccountStore::load(path).expect("load account store");
        let app = Arc::new(ServerApp::new(store));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let address = listener.local_addr().expect("local addr").to_string();

        let handle = thread::spawn(move || serve_listener(listener, app, Some(connection_limit)));

        SocketTestServer {
            _tmp: dir,
            address,
            handle,
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

    #[test]
    fn socket_client_can_create_account_login_and_list_tables() {
        let server = spawn_socket_test_server(1);
        let mut client = TestClient::connect(&server.address).expect("connect client");

        let create = client.send(RequestOp::CreateAccount {
            name: "alice".into(),
            pin: "1234".into(),
            amount: 500,
        });
        assert!(create.ok);

        let login = client.send(RequestOp::Login {
            name: "alice".into(),
            pin: "1234".into(),
        });
        assert!(login.ok);
        assert!(client.session_id.is_some());

        let tables = client.send(RequestOp::ListTables);
        assert!(tables.ok);
        match tables.payload.expect("tables payload") {
            ResponsePayload::Tables(tables) => {
                assert_eq!(tables.tables.len(), 1);
                assert_eq!(tables.tables[0].table_id, 1);
            }
            other => panic!("unexpected payload: {other:?}"),
        }

        drop(client);
        server.join();
    }

    #[test]
    fn socket_two_clients_can_join_same_table_and_read_views() {
        let server = spawn_socket_test_server(2);
        let mut alice = TestClient::connect(&server.address).expect("connect alice");
        let mut bob = TestClient::connect(&server.address).expect("connect bob");

        assert!(
            alice
                .send(RequestOp::CreateAccount {
                    name: "alice".into(),
                    pin: "1234".into(),
                    amount: 500,
                })
                .ok
        );
        assert!(
            bob.send(RequestOp::CreateAccount {
                name: "bob".into(),
                pin: "1234".into(),
                amount: 500,
            })
            .ok
        );

        assert!(
            alice
                .send(RequestOp::Login {
                    name: "alice".into(),
                    pin: "1234".into(),
                })
                .ok
        );
        assert!(
            bob.send(RequestOp::Login {
                name: "bob".into(),
                pin: "1234".into(),
            })
            .ok
        );

        assert!(alice.send(RequestOp::JoinTable { table_id: 1 }).ok);
        assert!(bob.send(RequestOp::JoinTable { table_id: 1 }).ok);
        assert!(alice.send(RequestOp::StartHand { table_id: 1 }).ok);

        let public_state = alice.send(RequestOp::GetTableState { table_id: 1 });
        match public_state.payload.expect("public state payload") {
            ResponsePayload::TableState(state) => {
                assert_eq!(state.table_id, 1);
                assert_eq!(state.seats.len(), 2);
                assert!(state.seats.iter().all(|seat| seat.hidden_card_count > 0));
            }
            other => panic!("unexpected payload: {other:?}"),
        }

        let alice_view = alice.send(RequestOp::GetPlayerView { table_id: 1 });
        let alice_prompt = match alice_view.payload.expect("alice player view") {
            ResponsePayload::PlayerView(view) => {
                assert_eq!(view.player_name, "alice");
                assert_eq!(view.private_cards.len(), 5);
                view.action_prompt
            }
            other => panic!("unexpected payload: {other:?}"),
        };

        let bob_view = bob.send(RequestOp::GetPlayerView { table_id: 1 });
        let bob_prompt = match bob_view.payload.expect("bob player view") {
            ResponsePayload::PlayerView(view) => {
                assert_eq!(view.player_name, "bob");
                assert_eq!(view.private_cards.len(), 5);
                view.action_prompt
            }
            other => panic!("unexpected payload: {other:?}"),
        };
        assert!(
            alice_prompt.is_some() || bob_prompt.is_some(),
            "one seated player should have the current action prompt"
        );

        drop(alice);
        drop(bob);
        server.join();
    }

    #[test]
    fn socket_relogin_invalidates_old_session() {
        let server = spawn_socket_test_server(2);
        let mut first_client = TestClient::connect(&server.address).expect("connect first client");
        let mut second_client =
            TestClient::connect(&server.address).expect("connect second client");

        assert!(
            first_client
                .send(RequestOp::CreateAccount {
                    name: "alice".into(),
                    pin: "1234".into(),
                    amount: 500,
                })
                .ok
        );

        let first_login = first_client.send(RequestOp::Login {
            name: "alice".into(),
            pin: "1234".into(),
        });
        assert!(first_login.ok);
        let first_session_id = first_client.session_id.clone().expect("first session");

        let second_login = second_client.send(RequestOp::Login {
            name: "alice".into(),
            pin: "1234".into(),
        });
        assert!(second_login.ok);
        let second_session_id = second_client.session_id.clone().expect("second session");
        assert_ne!(first_session_id, second_session_id);

        let stale = first_client.send(RequestOp::Deposit { amount: 10 });
        assert!(!stale.ok);
        let stale_error = stale.error.expect("stale session error");
        assert_eq!(stale_error.code, ErrorCode::SessionNotFound);

        let fresh = second_client.send(RequestOp::Deposit { amount: 10 });
        assert!(fresh.ok);
        match fresh.payload.expect("fresh deposit payload") {
            ResponsePayload::Account(account) => {
                assert_eq!(account.balance, 510);
            }
            other => panic!("unexpected payload: {other:?}"),
        }

        drop(first_client);
        drop(second_client);
        server.join();
    }

    #[test]
    fn socket_logout_invalidates_session_and_clears_waiting_table_seat() {
        let server = spawn_socket_test_server(1);
        let mut client = TestClient::connect(&server.address).expect("connect client");

        assert!(
            client
                .send(RequestOp::CreateAccount {
                    name: "alice".into(),
                    pin: "1234".into(),
                    amount: 500,
                })
                .ok
        );
        assert!(
            client
                .send(RequestOp::Login {
                    name: "alice".into(),
                    pin: "1234".into(),
                })
                .ok
        );
        let old_session_id = client.session_id.clone().expect("session id");
        assert!(client.send(RequestOp::JoinTable { table_id: 1 }).ok);

        let logout = client.send(RequestOp::Logout);
        assert!(logout.ok);
        assert!(client.session_id.is_none());

        let stale = client.send(RequestOp::Deposit { amount: 10 });
        assert!(!stale.ok);
        let stale_error = stale.error.expect("stale session error");
        assert_eq!(stale_error.code, ErrorCode::InvalidRequest);

        client.session_id = Some(old_session_id);
        let invalidated = client.send(RequestOp::GetPlayerView { table_id: 1 });
        assert!(!invalidated.ok);
        let invalidated_error = invalidated.error.expect("invalidated session error");
        assert_eq!(invalidated_error.code, ErrorCode::SessionNotFound);

        client.session_id = None;
        let state = client.send(RequestOp::GetTableState { table_id: 1 });
        assert!(state.ok);
        match state.payload.expect("table state payload") {
            ResponsePayload::TableState(state) => {
                assert!(state.seats.is_empty());
            }
            other => panic!("unexpected payload: {other:?}"),
        }

        drop(client);
        server.join();
    }

    #[test]
    fn socket_start_hand_requests_on_different_tables_can_progress_independently() {
        let server = spawn_socket_test_server(4);
        let mut alice = TestClient::connect(&server.address).expect("connect alice");
        let mut bob = TestClient::connect(&server.address).expect("connect bob");
        let mut carol = TestClient::connect(&server.address).expect("connect carol");
        let mut dave = TestClient::connect(&server.address).expect("connect dave");

        for (client, name) in [
            (&mut alice, "alice"),
            (&mut bob, "bob"),
            (&mut carol, "carol"),
            (&mut dave, "dave"),
        ] {
            assert!(
                client
                    .send(RequestOp::CreateAccount {
                        name: name.into(),
                        pin: "1234".into(),
                        amount: 500,
                    })
                    .ok
            );
            assert!(
                client
                    .send(RequestOp::Login {
                        name: name.into(),
                        pin: "1234".into(),
                    })
                    .ok
            );
        }

        let second_table_id = match alice
            .send(RequestOp::CreateTable {
                variant: PokerVariant::TexasHoldEm,
            })
            .payload
            .expect("create table payload")
        {
            ResponsePayload::Table(table) => table.table_id,
            other => panic!("unexpected payload: {other:?}"),
        };

        assert!(alice.send(RequestOp::JoinTable { table_id: 1 }).ok);
        assert!(bob.send(RequestOp::JoinTable { table_id: 1 }).ok);
        assert!(
            carol
                .send(RequestOp::JoinTable {
                    table_id: second_table_id,
                })
                .ok
        );
        assert!(
            dave.send(RequestOp::JoinTable {
                table_id: second_table_id,
            })
            .ok
        );

        let barrier = Arc::new(Barrier::new(2));
        let barrier_a = Arc::clone(&barrier);
        let barrier_b = Arc::clone(&barrier);

        let start_table_one = thread::spawn(move || {
            barrier_a.wait();
            alice.send(RequestOp::StartHand { table_id: 1 })
        });

        let start_table_two = thread::spawn(move || {
            barrier_b.wait();
            carol.send(RequestOp::StartHand {
                table_id: second_table_id,
            })
        });

        let start_one = start_table_one.join().expect("table 1 start thread");
        let start_two = start_table_two.join().expect("table 2 start thread");
        assert!(start_one.ok, "table 1 start should succeed: {start_one:?}");
        assert!(start_two.ok, "table 2 start should succeed: {start_two:?}");

        let table_one_state = bob.send(RequestOp::GetTableState { table_id: 1 });
        match table_one_state.payload.expect("table 1 state payload") {
            ResponsePayload::TableState(state) => {
                assert_eq!(state.table_id, 1);
                assert_eq!(state.seats.len(), 2);
                assert_ne!(state.step, HandStep::WaitingForPlayers);
            }
            other => panic!("unexpected payload: {other:?}"),
        }

        let table_two_state = dave.send(RequestOp::GetTableState {
            table_id: second_table_id,
        });
        match table_two_state.payload.expect("table 2 state payload") {
            ResponsePayload::TableState(state) => {
                assert_eq!(state.table_id, second_table_id);
                assert_eq!(state.variant, PokerVariant::TexasHoldEm);
                assert_eq!(state.seats.len(), 2);
                assert_ne!(state.step, HandStep::WaitingForPlayers);
            }
            other => panic!("unexpected payload: {other:?}"),
        }

        drop(bob);
        drop(dave);
        server.join();
    }
}
