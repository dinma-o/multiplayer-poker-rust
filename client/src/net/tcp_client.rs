use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::state::{ClientEvent, EventHandler};
use common::wire::{
    ClientRequest, RequestId, RequestOp, ResponsePayload, ServerMessage, ServerResponse, SessionId,
};

type PendingMap = Arc<Mutex<HashMap<RequestId, Sender<ServerResponse>>>>;
type SharedSession = Arc<Mutex<Option<SessionId>>>;

/// Thin newline-delimited TCP client with a background reader thread.
///
/// Requests are sent synchronously by `send`, while the reader thread demultiplexes responses and
/// push messages back onto channels/callbacks for the rest of the client.
pub struct TcpClient {
    writer: TcpStream,
    next_request_id: u64,
    pending: PendingMap,
    session_id: SharedSession,
}

impl TcpClient {
    pub fn connect(address: &str, event_handler: Option<EventHandler>) -> io::Result<Self> {
        let writer = TcpStream::connect(address)?;
        let read_stream = writer.try_clone()?;
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let session_id = Arc::new(Mutex::new(None));

        spawn_reader_thread(
            read_stream,
            Arc::clone(&pending),
            Arc::clone(&session_id),
            event_handler,
        );

        Ok(Self {
            writer,
            next_request_id: 1,
            pending,
            session_id,
        })
    }

    /// Sends one request and waits for the matching response while the reader thread continues to
    /// process push updates in the background.
    pub fn send(&mut self, op: RequestOp) -> io::Result<ServerResponse> {
        let request_id = format!("req-{}", self.next_request_id);
        self.next_request_id = self.next_request_id.saturating_add(1);

        let request = ClientRequest {
            request_id: request_id.clone(),
            session_id: self.session_id(),
            op,
        };

        let json = serde_json::to_string(&request).map_err(io::Error::other)?;

        let (reply_tx, reply_rx) = mpsc::channel();
        self.pending
            .lock()
            .expect("pending request map lock poisoned")
            .insert(request_id.clone(), reply_tx);
        if let Err(error) = self.writer.write_all(json.as_bytes()) {
            self.pending
                .lock()
                .expect("pending request map lock poisoned")
                .remove(&request_id);
            return Err(error);
        }
        if let Err(error) = self.writer.write_all(b"\n") {
            self.pending
                .lock()
                .expect("pending request map lock poisoned")
                .remove(&request_id);
            return Err(error);
        }
        if let Err(error) = self.writer.flush() {
            self.pending
                .lock()
                .expect("pending request map lock poisoned")
                .remove(&request_id);
            return Err(error);
        }

        reply_rx.recv().map_err(|_| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "server connection closed before a response was received",
            )
        })
    }

    pub fn session_id(&self) -> Option<SessionId> {
        self.session_id
            .lock()
            .expect("session lock poisoned")
            .clone()
    }
}

/// Demultiplexes newline-delimited server messages into reply channels and push events.
fn spawn_reader_thread(
    read_stream: TcpStream,
    pending: PendingMap,
    session_id: SharedSession,
    event_handler: Option<EventHandler>,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(read_stream);

        loop {
            let mut line = String::new();
            let bytes_read = match reader.read_line(&mut line) {
                Ok(bytes_read) => bytes_read,
                Err(error) => {
                    emit_event(
                        &event_handler,
                        ClientEvent::TransportError(format!("server read error: {error}")),
                    );
                    break;
                }
            };
            if bytes_read == 0 {
                emit_event(&event_handler, ClientEvent::Disconnected);
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let message: ServerMessage = match serde_json::from_str(trimmed) {
                Ok(message) => message,
                Err(error) => {
                    emit_event(
                        &event_handler,
                        ClientEvent::TransportError(format!(
                            "failed to decode server message: {error}"
                        )),
                    );
                    continue;
                }
            };

            match message {
                ServerMessage::Response { response } => {
                    capture_session(&session_id, &response);
                    if let Some(reply_tx) = pending
                        .lock()
                        .expect("pending request map lock poisoned")
                        .remove(&response.request_id)
                    {
                        let _ = reply_tx.send(response);
                    } else {
                        // Push responses without a waiting caller are unexpected at the GUI layer
                        // and usually point to a request/response ordering bug.
                        emit_event(&event_handler, ClientEvent::UnexpectedResponse(response));
                    }
                }
                ServerMessage::TableUpdate(update) => {
                    emit_event(&event_handler, ClientEvent::TableUpdate(update));
                }
                ServerMessage::SessionClosed(message) => {
                    *session_id.lock().expect("session lock poisoned") = None;
                    emit_event(&event_handler, ClientEvent::SessionClosed(message));
                }
            }
        }

        let mut pending = pending.lock().expect("pending request map lock poisoned");
        pending.clear();
    });
}

fn emit_event(event_handler: &Option<EventHandler>, event: ClientEvent) {
    if let Some(handler) = event_handler {
        handler(event);
    }
}

/// Mirrors server-owned authentication state locally so future requests automatically carry the
/// latest `session_id`.
fn capture_session(session_id: &SharedSession, response: &ServerResponse) {
    if !response.ok {
        return;
    }

    match &response.payload {
        Some(ResponsePayload::Login(login)) => {
            *session_id.lock().expect("session lock poisoned") = Some(login.session_id.clone());
        }
        Some(ResponsePayload::Logout(_)) => {
            *session_id.lock().expect("session lock poisoned") = None;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::wire::{AccountProfile, ErrorCode, ErrorPayload, LoginResult, LogoutResult};

    #[test]
    fn capture_session_sets_session_on_login_response() {
        let session_id = Arc::new(Mutex::new(None));
        let response = ServerResponse::ok(
            "req-1",
            ResponsePayload::Login(LoginResult {
                session_id: "session-42".into(),
                account: AccountProfile {
                    player_id: 7,
                    player_name: "alice".into(),
                    balance: 500,
                    games_played: 0,
                    pots_won: 0,
                    folds: 0,
                },
            }),
        );

        capture_session(&session_id, &response);

        assert_eq!(
            session_id.lock().expect("session lock").as_deref(),
            Some("session-42")
        );
    }

    #[test]
    fn capture_session_clears_session_on_logout_response() {
        let session_id = Arc::new(Mutex::new(Some("session-42".into())));
        let response = ServerResponse::ok(
            "req-2",
            ResponsePayload::Logout(LogoutResult {
                session_id: "session-42".into(),
            }),
        );

        capture_session(&session_id, &response);

        assert_eq!(*session_id.lock().expect("session lock"), None);
    }

    #[test]
    fn capture_session_ignores_error_responses() {
        let session_id = Arc::new(Mutex::new(Some("session-42".into())));
        let response = ServerResponse::error(
            "req-3",
            ErrorPayload::new(ErrorCode::AuthenticationFailed, "bad login"),
        );

        capture_session(&session_id, &response);

        assert_eq!(
            session_id.lock().expect("session lock").as_deref(),
            Some("session-42")
        );
    }
}
