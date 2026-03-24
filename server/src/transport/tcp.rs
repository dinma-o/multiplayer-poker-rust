use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use common::wire::{
    ClientRequest, ErrorCode, ErrorPayload, RequestOp, ResponsePayload, ServerMessage,
    ServerResponse, SessionCloseReason, SubscriptionResult, TableId, TableUpdateMessage,
};

use crate::app::{RequestOutcome, ServerApp, TableChangeNotice};

pub const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";

pub type SharedServerApp = Arc<ServerApp>;
type SharedConnectionRegistry = Arc<Mutex<ConnectionRegistry>>;
type ConnectionId = u64;

#[derive(Debug, Clone)]
struct ConnectionRecord {
    sender: Sender<ServerMessage>,
    session_id: Option<String>,
    subscribed_tables: HashSet<TableId>,
}

#[derive(Debug, Default)]
struct ConnectionRegistry {
    next_connection_id: ConnectionId,
    connections: HashMap<ConnectionId, ConnectionRecord>,
}

impl ConnectionRegistry {
    fn insert(&mut self, sender: Sender<ServerMessage>) -> ConnectionId {
        self.next_connection_id = self.next_connection_id.saturating_add(1);
        let connection_id = self.next_connection_id;
        self.connections.insert(
            connection_id,
            ConnectionRecord {
                sender,
                session_id: None,
                subscribed_tables: HashSet::new(),
            },
        );
        connection_id
    }

    fn remove(&mut self, connection_id: ConnectionId) -> Option<ConnectionRecord> {
        self.connections.remove(&connection_id)
    }

    fn set_session(&mut self, connection_id: ConnectionId, session_id: Option<String>) {
        if let Some(record) = self.connections.get_mut(&connection_id) {
            record.session_id = session_id;
            if record.session_id.is_none() {
                record.subscribed_tables.clear();
            }
        }
    }

    fn subscribe(&mut self, connection_id: ConnectionId, table_id: TableId) {
        if let Some(record) = self.connections.get_mut(&connection_id) {
            record.subscribed_tables.insert(table_id);
        }
    }

    fn unsubscribe(&mut self, connection_id: ConnectionId, table_id: TableId) {
        if let Some(record) = self.connections.get_mut(&connection_id) {
            record.subscribed_tables.remove(&table_id);
        }
    }

    fn has_active_session_elsewhere(&self, session_id: &str) -> bool {
        self.connections
            .values()
            .any(|record| record.session_id.as_deref() == Some(session_id))
    }

    fn invalidate_session(&mut self, session_id: &str) -> Vec<InvalidatedConnection> {
        let mut invalidated = Vec::new();
        for (connection_id, record) in &mut self.connections {
            if record.session_id.as_deref() == Some(session_id) {
                record.session_id = None;
                record.subscribed_tables.clear();
                invalidated.push(InvalidatedConnection {
                    connection_id: *connection_id,
                    session_id: session_id.to_string(),
                    sender: record.sender.clone(),
                });
            }
        }
        invalidated
    }

    fn subscribers_for_table(&self, table_id: TableId) -> Vec<SubscriptionTarget> {
        self.connections
            .iter()
            .filter_map(|(connection_id, record)| {
                record
                    .subscribed_tables
                    .contains(&table_id)
                    .then(|| SubscriptionTarget {
                        connection_id: *connection_id,
                        sender: record.sender.clone(),
                        session_id: record.session_id.clone(),
                    })
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct SubscriptionTarget {
    connection_id: ConnectionId,
    sender: Sender<ServerMessage>,
    session_id: Option<String>,
}

#[derive(Debug, Clone)]
struct InvalidatedConnection {
    connection_id: ConnectionId,
    session_id: String,
    sender: Sender<ServerMessage>,
}

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
    let registry = Arc::new(Mutex::new(ConnectionRegistry::default()));
    let mut handler_threads = Vec::new();
    let mut accepted_connections = 0usize;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer = stream.peer_addr().ok();
                let app = Arc::clone(&app);
                let registry = Arc::clone(&registry);
                let handler = thread::spawn(move || {
                    if let Err(error) = handle_client(stream, app, registry) {
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

fn handle_client(
    stream: TcpStream,
    app: SharedServerApp,
    registry: SharedConnectionRegistry,
) -> io::Result<()> {
    let read_stream = stream.try_clone()?;
    let mut reader = BufReader::new(read_stream);
    let (writer_tx, writer_rx) = mpsc::channel();
    let connection_id = registry
        .lock()
        .expect("connection registry lock poisoned")
        .insert(writer_tx.clone());

    let writer_handle = thread::spawn(move || writer_loop(stream, writer_rx));
    let result = reader_loop(&mut reader, connection_id, &writer_tx, &app, &registry);

    cleanup_connection(connection_id, &app, &registry);
    drop(writer_tx);

    match writer_handle.join() {
        Ok(Ok(())) => {}
        Ok(Err(error)) if result.is_ok() => {
            return Err(error);
        }
        Ok(Err(_)) | Err(_) => {}
    }

    result
}

fn writer_loop(mut stream: TcpStream, rx: Receiver<ServerMessage>) -> io::Result<()> {
    while let Ok(message) = rx.recv() {
        write_message(&mut stream, &message)?;
    }

    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

fn reader_loop(
    reader: &mut BufReader<TcpStream>,
    connection_id: ConnectionId,
    writer_tx: &Sender<ServerMessage>,
    app: &SharedServerApp,
    registry: &SharedConnectionRegistry,
) -> io::Result<()> {
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

        let request: ClientRequest = match serde_json::from_str(trimmed) {
            Ok(request) => request,
            Err(error) => {
                send_message(
                    writer_tx,
                    ServerMessage::response(ServerResponse::error(
                        "invalid-request",
                        ErrorPayload::new(
                            ErrorCode::InvalidRequest,
                            format!("invalid request JSON: {error}"),
                        ),
                    )),
                )?;
                continue;
            }
        };

        handle_client_request(connection_id, request, writer_tx, app, registry)?;
    }
}

fn handle_client_request(
    connection_id: ConnectionId,
    request: ClientRequest,
    writer_tx: &Sender<ServerMessage>,
    app: &SharedServerApp,
    registry: &SharedConnectionRegistry,
) -> io::Result<()> {
    match request.op.clone() {
        RequestOp::SubscribeTable { table_id } => {
            handle_subscribe_request(connection_id, request, table_id, writer_tx, app, registry)
        }
        RequestOp::UnsubscribeTable { table_id } => {
            handle_unsubscribe_request(connection_id, request, table_id, writer_tx, registry)
        }
        _ => {
            let outcome = app.handle_request_outcome(request);
            process_request_outcome(connection_id, outcome, writer_tx, app, registry)
        }
    }
}

fn handle_subscribe_request(
    connection_id: ConnectionId,
    request: ClientRequest,
    table_id: TableId,
    writer_tx: &Sender<ServerMessage>,
    app: &SharedServerApp,
    registry: &SharedConnectionRegistry,
) -> io::Result<()> {
    match app.get_table_state(table_id) {
        Ok(_) => {
            registry
                .lock()
                .expect("connection registry lock poisoned")
                .subscribe(connection_id, table_id);

            send_message(
                writer_tx,
                ServerMessage::response(ServerResponse::ok(
                    request.request_id,
                    ResponsePayload::Subscription(SubscriptionResult {
                        table_id,
                        subscribed: true,
                    }),
                )),
            )?;

            let update = app
                .build_table_update_for_session(table_id, request.session_id.as_deref(), &[])
                .map_err(|error| io::Error::other(error.message))?;
            send_message(writer_tx, ServerMessage::table_update(update))
        }
        Err(error) => send_message(
            writer_tx,
            ServerMessage::response(ServerResponse::error(request.request_id, error)),
        ),
    }
}

fn handle_unsubscribe_request(
    connection_id: ConnectionId,
    request: ClientRequest,
    table_id: TableId,
    writer_tx: &Sender<ServerMessage>,
    registry: &SharedConnectionRegistry,
) -> io::Result<()> {
    registry
        .lock()
        .expect("connection registry lock poisoned")
        .unsubscribe(connection_id, table_id);

    send_message(
        writer_tx,
        ServerMessage::response(ServerResponse::ok(
            request.request_id,
            ResponsePayload::Subscription(SubscriptionResult {
                table_id,
                subscribed: false,
            }),
        )),
    )
}

fn process_request_outcome(
    connection_id: ConnectionId,
    outcome: RequestOutcome,
    writer_tx: &Sender<ServerMessage>,
    app: &SharedServerApp,
    registry: &SharedConnectionRegistry,
) -> io::Result<()> {
    let response = outcome.response.clone();
    send_message(writer_tx, ServerMessage::response(response.clone()))?;

    apply_connection_response_state(connection_id, &response, registry);
    invalidate_sessions(outcome.invalidated_sessions, registry, app);
    broadcast_table_changes(app, registry, &outcome.table_changes);
    Ok(())
}

fn apply_connection_response_state(
    connection_id: ConnectionId,
    response: &ServerResponse,
    registry: &SharedConnectionRegistry,
) {
    if !response.ok {
        return;
    }

    let mut registry = registry.lock().expect("connection registry lock poisoned");
    match &response.payload {
        Some(ResponsePayload::Login(login)) => {
            registry.set_session(connection_id, Some(login.session_id.clone()));
        }
        Some(ResponsePayload::Logout(_)) => {
            registry.set_session(connection_id, None);
        }
        Some(ResponsePayload::JoinTable(join)) => {
            registry.subscribe(connection_id, join.table_id);
        }
        Some(ResponsePayload::LeaveTable(leave)) => {
            registry.unsubscribe(connection_id, leave.table_id);
        }
        _ => {}
    }
}

fn invalidate_sessions(
    session_ids: Vec<String>,
    registry: &SharedConnectionRegistry,
    app: &SharedServerApp,
) {
    if session_ids.is_empty() {
        return;
    }

    let invalidated = {
        let mut registry = registry.lock().expect("connection registry lock poisoned");
        let mut invalidated = Vec::new();
        for session_id in &session_ids {
            invalidated.extend(registry.invalidate_session(session_id));
        }
        invalidated
    };

    for invalidated_connection in invalidated {
        let message = ServerMessage::session_closed(
            invalidated_connection.session_id.clone(),
            SessionCloseReason::ReplacedByLogin,
            "session replaced by a new login",
        );
        if invalidated_connection.sender.send(message).is_err() {
            cleanup_connection(invalidated_connection.connection_id, app, registry);
        }
    }
}

fn broadcast_table_changes(
    app: &SharedServerApp,
    registry: &SharedConnectionRegistry,
    table_changes: &[TableChangeNotice],
) {
    let mut failed_connection_ids = Vec::new();

    for table_change in table_changes {
        let subscribers = {
            registry
                .lock()
                .expect("connection registry lock poisoned")
                .subscribers_for_table(table_change.table_id)
        };

        for subscriber in subscribers {
            let update = match app.build_table_update_for_session(
                table_change.table_id,
                subscriber.session_id.as_deref(),
                &table_change.events,
            ) {
                Ok(update) => update,
                Err(_) => {
                    continue;
                }
            };

            if subscriber
                .sender
                .send(ServerMessage::table_update(update))
                .is_err()
            {
                failed_connection_ids.push(subscriber.connection_id);
            }
        }
    }

    for connection_id in failed_connection_ids {
        cleanup_connection(connection_id, app, registry);
    }
}

fn cleanup_connection(
    connection_id: ConnectionId,
    app: &SharedServerApp,
    registry: &SharedConnectionRegistry,
) {
    let session_to_disconnect = {
        let mut registry = registry.lock().expect("connection registry lock poisoned");
        let removed = match registry.remove(connection_id) {
            Some(removed) => removed,
            None => {
                return;
            }
        };

        removed
            .session_id
            .filter(|session_id| !registry.has_active_session_elsewhere(session_id))
    };

    if let Some(session_id) = session_to_disconnect {
        let table_changes = app.handle_disconnect(&session_id);
        broadcast_table_changes(app, registry, &table_changes);
    }
}

fn handle_request_line(line: &str, app: &SharedServerApp) -> ServerMessage {
    let request: ClientRequest = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(error) => {
            return ServerMessage::response(ServerResponse::error(
                "invalid-request",
                ErrorPayload::new(
                    ErrorCode::InvalidRequest,
                    format!("invalid request JSON: {error}"),
                ),
            ));
        }
    };

    ServerMessage::response(app.handle_request(request))
}

fn send_message(sender: &Sender<ServerMessage>, message: ServerMessage) -> io::Result<()> {
    sender.send(message).map_err(|_| {
        io::Error::new(
            io::ErrorKind::BrokenPipe,
            "connection writer is unavailable",
        )
    })
}

fn write_message(stream: &mut TcpStream, message: &ServerMessage) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, message).map_err(io::Error::other)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account_store::AccountStore;
    use common::protocol::{HandStep, PokerVariant};
    use common::wire::{
        ClientRequest, RequestOp, ResponsePayload, SessionClosedMessage, SubscriptionResult,
    };
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
        pending_updates: Vec<TableUpdateMessage>,
        pending_session_closed: Vec<SessionClosedMessage>,
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
                pending_updates: Vec::new(),
                pending_session_closed: Vec::new(),
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

            loop {
                match self.read_message() {
                    ServerMessage::Response { response } => {
                        self.capture_session(&response);
                        return response;
                    }
                    ServerMessage::TableUpdate(update) => self.pending_updates.push(update),
                    ServerMessage::SessionClosed(message) => {
                        self.session_id = None;
                        self.pending_session_closed.push(message);
                    }
                }
            }
        }

        fn read_message(&mut self) -> ServerMessage {
            let mut line = String::new();
            self.reader
                .read_line(&mut line)
                .expect("read server message line");
            serde_json::from_str(line.trim()).expect("deserialize server message")
        }

        fn wait_for_table_update(&mut self, table_id: TableId) -> TableUpdateMessage {
            if let Some(index) = self
                .pending_updates
                .iter()
                .position(|update| update.table_id == table_id)
            {
                return self.pending_updates.remove(index);
            }

            loop {
                match self.read_message() {
                    ServerMessage::Response { response } => {
                        panic!("unexpected response while waiting for table update: {response:?}");
                    }
                    ServerMessage::TableUpdate(update) if update.table_id == table_id => {
                        return update;
                    }
                    ServerMessage::TableUpdate(update) => self.pending_updates.push(update),
                    ServerMessage::SessionClosed(message) => {
                        self.session_id = None;
                        self.pending_session_closed.push(message);
                    }
                }
            }
        }

        fn wait_for_table_update_matching<F>(
            &mut self,
            table_id: TableId,
            predicate: F,
        ) -> TableUpdateMessage
        where
            F: Fn(&TableUpdateMessage) -> bool,
        {
            if let Some(index) = self
                .pending_updates
                .iter()
                .position(|update| update.table_id == table_id && predicate(update))
            {
                return self.pending_updates.remove(index);
            }

            loop {
                match self.read_message() {
                    ServerMessage::Response { response } => {
                        panic!("unexpected response while waiting for table update: {response:?}");
                    }
                    ServerMessage::TableUpdate(update)
                        if update.table_id == table_id && predicate(&update) =>
                    {
                        return update;
                    }
                    ServerMessage::TableUpdate(update) => self.pending_updates.push(update),
                    ServerMessage::SessionClosed(message) => {
                        self.session_id = None;
                        self.pending_session_closed.push(message);
                    }
                }
            }
        }

        fn wait_for_session_closed(&mut self) -> SessionClosedMessage {
            if let Some(message) = self.pending_session_closed.pop() {
                return message;
            }

            loop {
                match self.read_message() {
                    ServerMessage::Response { response } => {
                        panic!(
                            "unexpected response while waiting for session closed: {response:?}"
                        );
                    }
                    ServerMessage::TableUpdate(update) => self.pending_updates.push(update),
                    ServerMessage::SessionClosed(message) => {
                        self.session_id = None;
                        return message;
                    }
                }
            }
        }

        fn capture_session(&mut self, response: &ServerResponse) {
            if !response.ok {
                return;
            }

            match &response.payload {
                Some(ResponsePayload::Login(login)) => {
                    self.session_id = Some(login.session_id.clone());
                }
                Some(ResponsePayload::Logout(_)) => {
                    self.session_id = None;
                }
                _ => {}
            }
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
        match response {
            ServerMessage::Response { response } => {
                assert!(!response.ok);
                let error = response.error.expect("error payload");
                assert_eq!(error.code, ErrorCode::InvalidRequest);
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn valid_json_routes_to_server_app() {
        let fixture = test_fixture();
        let response = handle_request_line(
            r#"{"request_id":"req-1","session_id":null,"op":{"type":"list_tables"}}"#,
            &fixture.app,
        );

        match response {
            ServerMessage::Response { response } => {
                assert!(response.ok);
                match response.payload.expect("payload") {
                    ResponsePayload::Tables(tables) => assert_eq!(tables.tables.len(), 1),
                    other => panic!("unexpected payload: {other:?}"),
                }
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn response_writer_uses_newline_delimited_json() {
        let mut bytes = Vec::new();
        let message = ServerMessage::response(ServerResponse::ok(
            "req-2",
            ResponsePayload::Tables(
                crate::app::ServerApp::new(
                    AccountStore::load(tempdir().expect("tempdir").path().join("accounts.json"))
                        .expect("load account store"),
                )
                .list_tables(),
            ),
        ));

        serde_json::to_writer(&mut bytes, &message).expect("serialize response");
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
    fn socket_table_action_pushes_updates_to_other_seated_players() {
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

        let _ = alice.wait_for_table_update_matching(1, |update| {
            update.table_state.step != HandStep::WaitingForPlayers
        });
        let _ = bob.wait_for_table_update_matching(1, |update| {
            update.table_state.step != HandStep::WaitingForPlayers
        });
        alice.pending_updates.clear();
        bob.pending_updates.clear();

        let state = alice.send(RequestOp::GetTableState { table_id: 1 });
        let acting_player_id = match state.payload.expect("table state payload") {
            ResponsePayload::TableState(state) => state.acting_player_id.expect("acting player"),
            other => panic!("unexpected payload: {other:?}"),
        };

        let acting_view = if acting_player_id == 1 {
            alice.send(RequestOp::GetPlayerView { table_id: 1 })
        } else {
            bob.send(RequestOp::GetPlayerView { table_id: 1 })
        };
        let action = match acting_view.payload.expect("player view payload") {
            ResponsePayload::PlayerView(view) => {
                let allowed = view.action_prompt.expect("action prompt").allowed_actions;
                if allowed.contains(&common::protocol::AllowedAction::Check) {
                    common::wire::PlayerAction::Check
                } else {
                    common::wire::PlayerAction::Call
                }
            }
            other => panic!("unexpected payload: {other:?}"),
        };

        let pushed_update = if acting_player_id == 1 {
            assert!(
                alice
                    .send(RequestOp::SubmitAction {
                        table_id: 1,
                        action: action.clone(),
                    })
                    .ok
            );
            bob.wait_for_table_update_matching(1, |update| !update.events.is_empty())
        } else {
            assert!(
                bob.send(RequestOp::SubmitAction {
                    table_id: 1,
                    action,
                })
                .ok
            );
            alice.wait_for_table_update_matching(1, |update| !update.events.is_empty())
        };
        assert_eq!(pushed_update.table_id, 1);
        assert!(!pushed_update.events.is_empty());
        assert!(pushed_update.player_view.is_some());
        assert_eq!(pushed_update.table_state.seats.len(), 2);

        drop(alice);
        drop(bob);
        server.join();
    }

    #[test]
    fn socket_subscribed_spectator_receives_public_only_updates() {
        let server = spawn_socket_test_server(3);
        let mut alice = TestClient::connect(&server.address).expect("connect alice");
        let mut bob = TestClient::connect(&server.address).expect("connect bob");
        let mut spectator = TestClient::connect(&server.address).expect("connect spectator");

        for (client, name) in [(&mut alice, "alice"), (&mut bob, "bob")] {
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

        assert!(alice.send(RequestOp::JoinTable { table_id: 1 }).ok);
        assert!(bob.send(RequestOp::JoinTable { table_id: 1 }).ok);

        let subscribe = spectator.send(RequestOp::SubscribeTable { table_id: 1 });
        match subscribe.payload.expect("subscription payload") {
            ResponsePayload::Subscription(SubscriptionResult {
                table_id,
                subscribed,
            }) => {
                assert_eq!(table_id, 1);
                assert!(subscribed);
            }
            other => panic!("unexpected payload: {other:?}"),
        }

        let initial_update = spectator.wait_for_table_update(1);
        assert!(initial_update.player_view.is_none());

        assert!(alice.send(RequestOp::StartHand { table_id: 1 }).ok);
        let spectator_update = spectator.wait_for_table_update_matching(1, |update| {
            update.table_state.step != HandStep::WaitingForPlayers
        });
        assert_eq!(spectator_update.table_id, 1);
        assert!(spectator_update.player_view.is_none());
        assert_eq!(spectator_update.table_state.seats.len(), 2);

        drop(alice);
        drop(bob);
        drop(spectator);
        server.join();
    }

    #[test]
    fn socket_relogin_invalidates_old_session_with_push_message() {
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
        assert!(
            first_client
                .send(RequestOp::Login {
                    name: "alice".into(),
                    pin: "1234".into(),
                })
                .ok
        );
        let old_session = first_client.session_id.clone().expect("old session");

        assert!(
            second_client
                .send(RequestOp::Login {
                    name: "alice".into(),
                    pin: "1234".into(),
                })
                .ok
        );
        let closed = first_client.wait_for_session_closed();
        assert_eq!(closed.session_id, old_session);
        assert_eq!(closed.reason, SessionCloseReason::ReplacedByLogin);
        assert!(first_client.session_id.is_none());

        drop(first_client);
        drop(second_client);
        server.join();
    }

    #[test]
    fn socket_disconnect_during_active_hand_marks_seat_disconnected_and_pushes_update() {
        let server = spawn_socket_test_server(2);
        let mut alice = TestClient::connect(&server.address).expect("connect alice");
        let mut bob = TestClient::connect(&server.address).expect("connect bob");

        for (client, name) in [(&mut alice, "alice"), (&mut bob, "bob")] {
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

        assert!(alice.send(RequestOp::JoinTable { table_id: 1 }).ok);
        assert!(bob.send(RequestOp::JoinTable { table_id: 1 }).ok);
        assert!(alice.send(RequestOp::StartHand { table_id: 1 }).ok);
        let _ = alice.wait_for_table_update_matching(1, |update| {
            update.table_state.step != HandStep::WaitingForPlayers
        });
        let _ = bob.wait_for_table_update_matching(1, |update| {
            update.table_state.step != HandStep::WaitingForPlayers
        });
        bob.pending_updates.clear();

        let _ = alice.writer.shutdown(Shutdown::Both);
        drop(alice);

        let update = bob.wait_for_table_update_matching(1, |update| {
            update
                .table_state
                .seats
                .iter()
                .any(|seat| !seat.is_connected)
                || update.table_state.seats.len() == 1
        });
        assert_eq!(update.table_id, 1);
        assert!(
            update
                .table_state
                .seats
                .iter()
                .any(|seat| !seat.is_connected)
                || update.table_state.seats.len() == 1,
            "disconnect should either show a disconnected seat or complete the hand and remove it"
        );

        drop(bob);
        server.join();
    }

    #[test]
    fn socket_disconnect_in_lobby_removes_seat_from_push_state() {
        let server = spawn_socket_test_server(2);
        let mut alice = TestClient::connect(&server.address).expect("connect alice");
        let mut bob = TestClient::connect(&server.address).expect("connect bob");

        for (client, name) in [(&mut alice, "alice"), (&mut bob, "bob")] {
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

        assert!(alice.send(RequestOp::JoinTable { table_id: 1 }).ok);
        assert!(bob.send(RequestOp::JoinTable { table_id: 1 }).ok);
        let _ = alice.wait_for_table_update(1);
        let _ = bob.wait_for_table_update(1);

        drop(alice);

        let update = bob.wait_for_table_update(1);
        assert_eq!(update.table_state.seats.len(), 1);
        assert!(
            update
                .table_state
                .seats
                .iter()
                .all(|seat| seat.is_connected)
        );

        drop(bob);
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

        assert!(alice.send(RequestOp::StartHand { table_id: 1 }).ok);
        assert!(
            carol
                .send(RequestOp::StartHand {
                    table_id: second_table_id,
                })
                .ok
        );

        let table_one_state = bob.wait_for_table_update_matching(1, |update| {
            update.table_state.step != HandStep::WaitingForPlayers
        });
        assert_eq!(table_one_state.table_id, 1);
        assert_ne!(
            table_one_state.table_state.step,
            HandStep::WaitingForPlayers
        );

        let table_two_state = dave.wait_for_table_update_matching(second_table_id, |update| {
            update.table_state.step != HandStep::WaitingForPlayers
        });
        assert_eq!(table_two_state.table_id, second_table_id);
        assert_eq!(
            table_two_state.table_state.variant,
            PokerVariant::TexasHoldEm
        );
        assert_ne!(
            table_two_state.table_state.step,
            HandStep::WaitingForPlayers
        );

        drop(alice);
        drop(bob);
        drop(carol);
        drop(dave);
        server.join();
    }
}
