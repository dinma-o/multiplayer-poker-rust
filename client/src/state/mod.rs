use std::sync::Arc;

use common::wire::{ServerResponse, SessionClosedMessage, TableUpdateMessage};

pub type EventHandler = Arc<dyn Fn(ClientEvent) + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub enum ClientEvent {
    UnexpectedResponse(ServerResponse),
    TableUpdate(TableUpdateMessage),
    SessionClosed(SessionClosedMessage),
    TransportError(String),
    Disconnected,
}
