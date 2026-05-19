//! WebSocket transport implementation for SIP.

use std::convert::Infallible;
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::net::SocketAddr;
use std::result::Result as StdResult;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::header::{
    CONNECTION, HeaderValue, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_PROTOCOL,
    SEC_WEBSOCKET_VERSION, UPGRADE,
};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::upgrade::Upgraded;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::Role;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use crate::error::{Error, Result};
use crate::transport::{
    Packet, SipTransport, Transport, TransportLayer, TransportMessage, TransportProtocol,
};

const SIP: HeaderValue = HeaderValue::from_static("sip");

type BytesBody = http_body_util::Full<hyper::body::Bytes>;

/// WebSocket transport for SIP communication.
///
/// This struct represents a WebSocket client connected to a WebSocket server,
/// communicating using the SIP WebSocket subprotocol.
pub struct WebSocketTransport {
    /// Bound address.
    local_addr: SocketAddr,
    /// The remote peer address.
    peer_addr: SocketAddr,
    /// The WebSocket sender used to send messages.
    sender: mpsc::Sender<WsMessage>,
}

impl WebSocketTransport {
    /// Establish WebSocket connection.
    pub async fn connect(
        url: impl IntoClientRequest,
        timeout: Duration,
        transports: &TransportLayer,
    ) -> Result<Transport> {
        let mut request = url.into_client_request().map_err(IoError::other)?;

        request.headers_mut().insert(SEC_WEBSOCKET_PROTOCOL, SIP);

        let (stream, _response) = tokio::time::timeout(timeout, connect_async(request))
            .await
            .map_err(|e| IoError::new(IoErrorKind::TimedOut, e))?
            .map_err(|e| {
                crate::Error::TransportError(format!("WebSocket Connection failed {e}!"))
            })?;

        let (local_addr, peer_addr) = match stream.get_ref() {
            MaybeTlsStream::Plain(tcp_stream) => {
                (tcp_stream.local_addr()?, tcp_stream.peer_addr()?)
            }
            MaybeTlsStream::Rustls(tls_stream) => {
                let (tcp_stream, _) = tls_stream.get_ref();
                (tcp_stream.local_addr()?, tcp_stream.peer_addr()?)
            }
            _ => return Err(IoError::other("Unsupported stream type"))?,
        };

        let (tx, rx) = mpsc::channel::<WsMessage>(1000);
        let ws_transport = WebSocketTransport {
            local_addr,
            peer_addr,
            sender: tx,
        };
        let transport = Transport::new(ws_transport);

        let transports_clone = transports.clone();
        let transport_clone = transport.clone();
        // Handle connection in separate task
        tokio::spawn(handle_ws_connection(
            peer_addr,
            transports_clone,
            transport_clone,
            stream,
            rx,
        ));

        Ok(transport)
    }

    /// Send a message over the WebSocket connection.
    async fn ws_send_msg(&self, msg: WsMessage) -> Result<()> {
        self.sender
            .send(msg)
            .await
            .map_err(|_| Error::ChannelClosed)
    }
}

#[async_trait]
impl SipTransport for WebSocketTransport {
    async fn send_msg(&self, buf: &[u8], _: &SocketAddr) -> Result<usize> {
        self.ws_send_msg(buf.into()).await?;

        Ok(buf.len())
    }

    fn remote_addr(&self) -> Option<SocketAddr> {
        Some(self.peer_addr)
    }

    fn protocol(&self) -> TransportProtocol {
        TransportProtocol::Ws
    }

    fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    fn is_reliable(&self) -> bool {
        true
    }

    fn is_secure(&self) -> bool {
        false
    }
}

/// A WebSocket listener that accepts incoming connections from WebSocket clients.
///
/// The [`WebSocketListener`] acts as a SIP WebSocket server. It accepts new TCP
/// connections and performs the WebSocket upgrade to the SIP WebSocket subprotocol.
pub struct WebSocketListener {
    /// Listener for TCP sockets.
    listener: TcpListener,
    /// The local address the listener is bound to.
    bind_addr: SocketAddr,
}

impl WebSocketListener {
    /// Creates a new `WebSocketListener`, which will be bound to the specified
    /// address.
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> Result<WebSocketListener> {
        let listener = TcpListener::bind(addr).await?;
        let bind_addr = listener.local_addr()?;
        Ok(Self {
            listener,
            bind_addr,
        })
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    /// Accepts incoming TCP connections and upgrade to a WebSocket connection.
    pub async fn accept_clients(self, transports: TransportLayer) -> Result<()> {
        loop {
            let (stream, remote_addr) = match self.listener.accept().await {
                Ok((stream, addr)) => (stream, addr),
                Err(e) => {
                    log::error!("failed to accept connection: {:?}", e);
                    continue;
                }
            };
            log::debug!("Got new possible websocket connection from {}", remote_addr);

            let local_addr = stream.local_addr()?;
            let transports = transports.clone();
            // Let's spawn the handling of each connection in a separate task.
            tokio::spawn(async move {
                let io = TokioIo::new(stream);

                let service = service_fn(move |req| {
                    Self::upgrade_to_websocket(req, transports.clone(), remote_addr, local_addr)
                });

                let conn = http1::Builder::new()
                    .serve_connection(io, service)
                    .with_upgrades();

                if let Err(err) = conn.await {
                    log::error!("failed to serve connection: {remote_addr} :{err:?}");
                }
            });
        }
    }

    async fn upgrade_to_websocket(
        request: Request<Incoming>,
        transports: TransportLayer,
        remote_addr: SocketAddr,
        local_addr: SocketAddr,
    ) -> StdResult<Response<BytesBody>, Infallible> {
        log::debug!("Received a new, potentially ws handshake");

        // Upgrade requests are only permitted on GET methods.
        if request.method() != hyper::Method::GET {
            return Ok(make_http_response(
                501,
                "Attempt to use unimplemented / unsupported method",
            ));
        }

        let headers = request.headers();

        if headers.get(UPGRADE).is_some_and(|v| v != "websocket") {
            return Ok(make_http_response(426, "Upgrade Required"));
        }

        if headers
            .get(SEC_WEBSOCKET_VERSION)
            .is_some_and(|v| v != "13")
        {
            return Ok(make_http_response(400, "Invalid Web Socket Version"));
        }

        if headers
            .get(SEC_WEBSOCKET_PROTOCOL)
            .is_some_and(|v| v != "sip")
        {
            return Ok(make_http_response(400, "Invalid WebSocket Protocol"));
        }

        let key = match headers.get(SEC_WEBSOCKET_KEY) {
            Some(key) => key.as_bytes(),
            None => return Ok(make_http_response(400, "The Sec-WebSocket-Key has missing")),
        };
        let accept_key = derive_accept_key(key);
        let version = request.version();

        tokio::spawn(async move {
            match hyper::upgrade::on(request).await {
                Ok(upgraded) => {
                    let upgraded = TokioIo::new(upgraded);
                    let ws_stream =
                        WebSocketStream::from_raw_socket(upgraded, Role::Server, None).await;
                    if let Err(err) =
                        Self::on_upgrade_completed(transports, remote_addr, local_addr, ws_stream)
                            .await
                    {
                        log::error!("Error on WebSocket: {:#?}", err);
                    }
                }
                Err(e) => log::debug!("upgrade error: {}", e),
            }
        });

        let upgrade = HeaderValue::from_static("websocket");
        let connection = HeaderValue::from_static("Upgrade");
        let accept: HeaderValue = accept_key.try_into().unwrap();

        let mut response = Response::new(BytesBody::default());

        *response.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
        *response.version_mut() = version;

        let headers_mut = response.headers_mut();

        headers_mut.append(UPGRADE, upgrade);
        headers_mut.append(CONNECTION, connection);
        headers_mut.append(SEC_WEBSOCKET_ACCEPT, accept);
        headers_mut.append(SEC_WEBSOCKET_PROTOCOL, SIP);

        Ok(response)
    }

    async fn on_upgrade_completed(
        transports: TransportLayer,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
        ws_stream: WebSocketStream<TokioIo<Upgraded>>,
    ) -> Result<()> {
        log::debug!("WebSocket connection established with: {}", peer_addr);
        let (tx, rx) = mpsc::channel::<WsMessage>(1000);

        let websocket = WebSocketTransport {
            local_addr,
            peer_addr,
            sender: tx,
        };
        let transport = Transport::new(websocket);

        // Handle connection.
        handle_ws_connection(peer_addr, transports, transport, ws_stream, rx).await;

        Ok(())
    }
}

async fn handle_ws_connection<S>(
    addr: SocketAddr,
    transports: TransportLayer,
    transport: Transport,
    stream: WebSocketStream<S>,
    mut rx: mpsc::Receiver<WsMessage>,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    log::debug!("Handling WS connection from {}", addr);

    let (mut send, mut recv) = stream.split();

    transports.register_transport(transport.key(), transport.clone());

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = send.send(msg).await {
                log::warn!("WebSocket send error: {e}");
                break;
            }
        }
        log::debug!("WebSocket send handler finished for {}", addr);
    });

    while let Some(ws_msg) = recv.next().await {
        let data = match ws_msg {
            Ok(WsMessage::Text(text)) => text.into(),
            Ok(WsMessage::Binary(bin)) => bin,
            Ok(WsMessage::Close(reason)) => {
                log::info!("Client closing connection: {:?}", reason);
                break;
            }
            Err(e) => {
                log::error!("Error receiving WebSocket message: {e}");
                return;
            }
            _ => {
                continue;
            }
        };

        let packet = Packet::new(data, addr);
        let transport = transport.clone();
        let msg = TransportMessage { transport, packet };

        transports.receive_message(msg);
    }

    log::info!("WebSocket connection disconnected: {}", addr);
    transports.remove_transport(&transport.key());
    send_task.abort();
}

fn make_http_response(status: u16, message: &'static str) -> Response<Full<bytes::Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain")
        .body(BytesBody::from(message))
        .unwrap()
}
