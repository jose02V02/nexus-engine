//! Central long-lived event infrastructure for Nexus Engine 1.02.
//!
//! QuickJS remains confined to the BrowserSession thread. Long-lived network
//! work (currently WebSockets) runs on a Tokio runtime thread and communicates
//! through typed commands/events. BrowserSession drains those events from
//! `tick()` and delivers them into the live JavaScript realm.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};
use std::str::FromStr;
use std::thread;

use futures_util::{SinkExt, StreamExt};
use tokio::runtime::Builder;
use tokio::sync::mpsc as tokio_mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSocketCommand {
    Open {
        id: u64,
        url: Url,
        origin: String,
        protocols: Vec<String>,
    },
    SendText { id: u64, text: String },
    SendBinary { id: u64, data: Vec<u8> },
    Close { id: u64, code: u16, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSocketEvent {
    Open { id: u64, protocol: Option<String> },
    Text { id: u64, text: String },
    Binary { id: u64, data: Vec<u8> },
    Error { id: u64, message: String },
    Closed { id: u64, code: u16, reason: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EventLoopStats {
    pub websocket_commands: usize,
    pub websocket_events: usize,
    pub active_websockets_hint: usize,
}

enum RuntimeCommand {
    WebSocket(WebSocketCommand),
    SocketFinished(u64),
    Reset,
    Shutdown,
}

enum SocketTaskCommand {
    SendText(String),
    SendBinary(Vec<u8>),
    Close(u16, String),
}

pub struct BrowserEventLoop {
    command_tx: tokio_mpsc::UnboundedSender<RuntimeCommand>,
    event_rx: Receiver<WebSocketEvent>,
    stats: EventLoopStats,
}

impl BrowserEventLoop {
    pub fn new() -> Result<Self, String> {
        let (command_tx, mut command_rx) = tokio_mpsc::unbounded_channel::<RuntimeCommand>();
        let runtime_command_tx = command_tx.clone();
        let (event_tx, event_rx) = mpsc::channel::<WebSocketEvent>();
        thread::Builder::new()
            .name("nexus-event-loop".to_owned())
            .spawn(move || {
                let runtime = match Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        eprintln!("Nexus event loop runtime failed: {error}");
                        return;
                    }
                };
                runtime.block_on(async move {
                    let mut sockets: HashMap<u64, tokio_mpsc::UnboundedSender<SocketTaskCommand>> = HashMap::new();
                    while let Some(command) = command_rx.recv().await {
                        match command {
                            RuntimeCommand::Shutdown => break,
                            RuntimeCommand::SocketFinished(id) => {
                                sockets.remove(&id);
                            }
                            RuntimeCommand::Reset => {
                                for (_, socket) in sockets.drain() {
                                    let _ = socket.send(SocketTaskCommand::Close(1001, "document navigated".to_owned()));
                                }
                            }
                            RuntimeCommand::WebSocket(WebSocketCommand::Open { id, url, origin, protocols }) => {
                                if sockets.contains_key(&id) {
                                    let _ = event_tx.send(WebSocketEvent::Error { id, message: "duplicate WebSocket id".to_owned() });
                                    continue;
                                }
                                let (socket_tx, socket_rx) = tokio_mpsc::unbounded_channel();
                                sockets.insert(id, socket_tx);
                                let events = event_tx.clone();
                                let finished = runtime_command_tx.clone();
                                tokio::spawn(async move {
                                    run_websocket(id, url, origin, protocols, socket_rx, events).await;
                                    let _ = finished.send(RuntimeCommand::SocketFinished(id));
                                });
                            }
                            RuntimeCommand::WebSocket(WebSocketCommand::SendText { id, text }) => {
                                if let Some(socket) = sockets.get(&id) {
                                    let _ = socket.send(SocketTaskCommand::SendText(text));
                                } else {
                                    let _ = event_tx.send(WebSocketEvent::Error { id, message: "WebSocket is not open".to_owned() });
                                }
                            }
                            RuntimeCommand::WebSocket(WebSocketCommand::SendBinary { id, data }) => {
                                if let Some(socket) = sockets.get(&id) {
                                    let _ = socket.send(SocketTaskCommand::SendBinary(data));
                                } else {
                                    let _ = event_tx.send(WebSocketEvent::Error { id, message: "WebSocket is not open".to_owned() });
                                }
                            }
                            RuntimeCommand::WebSocket(WebSocketCommand::Close { id, code, reason }) => {
                                if let Some(socket) = sockets.remove(&id) {
                                    let _ = socket.send(SocketTaskCommand::Close(code, reason));
                                }
                            }
                        }
                    }
                });
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            command_tx,
            event_rx,
            stats: EventLoopStats::default(),
        })
    }

    pub fn submit_websocket(&mut self, command: WebSocketCommand) -> Result<(), String> {
        self.command_tx
            .send(RuntimeCommand::WebSocket(command))
            .map_err(|_| "Nexus event loop has stopped".to_owned())?;
        self.stats.websocket_commands = self.stats.websocket_commands.saturating_add(1);
        Ok(())
    }

    pub fn reset_document(&mut self) {
        let _ = self.command_tx.send(RuntimeCommand::Reset);
        self.stats.active_websockets_hint = 0;
    }

    pub fn drain_websocket_events(&mut self, limit: usize) -> Vec<WebSocketEvent> {
        let mut events = Vec::new();
        for _ in 0..limit.max(1) {
            match self.event_rx.try_recv() {
                Ok(event) => {
                    match &event {
                        WebSocketEvent::Open { .. } => {
                            self.stats.active_websockets_hint = self.stats.active_websockets_hint.saturating_add(1);
                        }
                        WebSocketEvent::Closed { .. } => {
                            self.stats.active_websockets_hint = self.stats.active_websockets_hint.saturating_sub(1);
                        }
                        _ => {}
                    }
                    self.stats.websocket_events = self.stats.websocket_events.saturating_add(1);
                    events.push(event);
                }
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => break,
            }
        }
        events
    }

    #[must_use]
    pub fn stats(&self) -> EventLoopStats {
        self.stats
    }
}

impl Drop for BrowserEventLoop {
    fn drop(&mut self) {
        let _ = self.command_tx.send(RuntimeCommand::Shutdown);
    }
}

async fn run_websocket(
    id: u64,
    url: Url,
    origin: String,
    protocols: Vec<String>,
    mut command_rx: tokio_mpsc::UnboundedReceiver<SocketTaskCommand>,
    event_tx: mpsc::Sender<WebSocketEvent>,
) {
    let mut request = match url.as_str().into_client_request() {
        Ok(request) => request,
        Err(error) => {
            let _ = event_tx.send(WebSocketEvent::Error { id, message: error.to_string() });
            let _ = event_tx.send(WebSocketEvent::Closed { id, code: 1006, reason: "handshake request failed".to_owned() });
            return;
        }
    };
    if let Ok(value) = HeaderValue::from_str(&origin) {
        request.headers_mut().insert(HeaderName::from_static("origin"), value);
    }
    if !protocols.is_empty() {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(b"sec-websocket-protocol"),
            HeaderValue::from_str(&protocols.join(", ")),
        ) {
            request.headers_mut().insert(name, value);
        }
    }

    let (mut socket, response) = match connect_async(request).await {
        Ok(value) => value,
        Err(error) => {
            let _ = event_tx.send(WebSocketEvent::Error { id, message: error.to_string() });
            let _ = event_tx.send(WebSocketEvent::Closed { id, code: 1006, reason: "connection failed".to_owned() });
            return;
        }
    };
    let protocol = response
        .headers()
        .get("sec-websocket-protocol")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let _ = event_tx.send(WebSocketEvent::Open { id, protocol });

    loop {
        tokio::select! {
            outbound = command_rx.recv() => {
                let Some(outbound) = outbound else { break };
                let send_result = match outbound {
                    SocketTaskCommand::SendText(text) => socket.send(Message::Text(text.into())).await,
                    SocketTaskCommand::SendBinary(data) => socket.send(Message::Binary(data.into())).await,
                    SocketTaskCommand::Close(code, reason) => {
                        socket.send(Message::Close(Some(CloseFrame { code: code.into(), reason: reason.into() }))).await
                    }
                };
                if let Err(error) = send_result {
                    let _ = event_tx.send(WebSocketEvent::Error { id, message: error.to_string() });
                    let _ = event_tx.send(WebSocketEvent::Closed { id, code: 1006, reason: "send failed".to_owned() });
                    break;
                }
            }
            incoming = socket.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let _ = event_tx.send(WebSocketEvent::Text { id, text: text.to_string() });
                    }
                    Some(Ok(Message::Binary(data))) => {
                        let _ = event_tx.send(WebSocketEvent::Binary { id, data: data.to_vec() });
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if let Err(error) = socket.send(Message::Pong(data)).await {
                            let _ = event_tx.send(WebSocketEvent::Error { id, message: error.to_string() });
                            let _ = event_tx.send(WebSocketEvent::Closed { id, code: 1006, reason: "pong failed".to_owned() });
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(frame))) => {
                        let (code, reason) = frame.map_or((1000, String::new()), |frame| (u16::from(frame.code), frame.reason.to_string()));
                        let _ = event_tx.send(WebSocketEvent::Closed { id, code, reason });
                        break;
                    }
                    Some(Ok(Message::Frame(_))) => {}
                    Some(Err(error)) => {
                        let _ = event_tx.send(WebSocketEvent::Error { id, message: error.to_string() });
                        let _ = event_tx.send(WebSocketEvent::Closed { id, code: 1006, reason: "connection error".to_owned() });
                        break;
                    }
                    None => {
                        let _ = event_tx.send(WebSocketEvent::Closed { id, code: 1006, reason: "connection ended".to_owned() });
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_loop_can_start_and_accept_commands() {
        let mut loop_ = BrowserEventLoop::new().unwrap();
        loop_.submit_websocket(WebSocketCommand::Close { id: 77, code: 1000, reason: String::new() }).unwrap();
        assert_eq!(loop_.stats().websocket_commands, 1);
    }
}
