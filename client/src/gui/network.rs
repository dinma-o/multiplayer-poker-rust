use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use crate::net::TcpClient;
use crate::state::ClientEvent;
use common::wire::ServerResponse;

use super::app::GuiCommand;

/// Spawn the sender thread and wire up the push event handler.
///
/// Returns `(cmd_tx, resp_rx, push_rx)`:
/// - `cmd_tx`  — send `GuiCommand` from the UI thread to the sender thread
/// - `resp_rx` — receive `ServerResponse` back in the UI thread
/// - `push_rx` — receive unsolicited `ClientEvent` (TableUpdate, SessionClosed, …)
pub fn spawn_network_threads(
    address: &str,
    ctx: egui::Context,
) -> io::Result<(Sender<GuiCommand>, Receiver<ServerResponse>, Receiver<ClientEvent>)> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<GuiCommand>();
    let (resp_tx, resp_rx) = mpsc::channel::<ServerResponse>();
    let (push_tx, push_rx) = mpsc::channel::<ClientEvent>();

    // The push_tx clone goes into the EventHandler so the reader thread inside
    // TcpClient can forward unsolicited messages to the UI thread.
    let push_tx_handler = push_tx.clone();
    let ctx_handler = ctx.clone();
    let handler = Arc::new(move |event: ClientEvent| {
        let _ = push_tx_handler.send(event);
        ctx_handler.request_repaint();
    });

    let mut client = TcpClient::connect(address, Some(handler))?;

    // Sender thread: blocks on cmd_rx, calls client.send() (which is blocking),
    // and forwards the response to the UI thread via resp_tx.
    thread::spawn(move || {
        loop {
            let command = match cmd_rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => break, // cmd_tx was dropped — UI is gone
            };

            match command {
                GuiCommand::SendOp(op) => match client.send(op) {
                    Ok(response) => {
                        if resp_tx.send(response).is_err() {
                            break; // UI is gone
                        }
                    }
                    Err(e) => {
                        let _ = push_tx.send(ClientEvent::TransportError(e.to_string()));
                        ctx.request_repaint();
                        break;
                    }
                },
            }
        }
    });

    Ok((cmd_tx, resp_rx, push_rx))
}
