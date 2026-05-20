//! TCP transport implementation for SIP.

use std::net::SocketAddr;

use async_trait::async_trait;
use tokio::io::{AsyncWriteExt, ReadHalf, WriteHalf, split};
use tokio::net::{TcpListener as TokioTcpListener, TcpStream, ToSocketAddrs};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tokio_util::codec::FramedRead;

use super::decode::{FramedMessage, StreamingDecoder};
use super::{
    KEEPALIVE_RESPONSE, Packet, SipTransport, Transport, TransportMessage, TransportProtocol,
};
use crate::error::{Error, Result};
use crate::transport::TransportLayer;

/// TCP transport implementation.
///
/// The [`TcpTransport`] represents a single reliable, connection-oriented transport
/// between a local and a remote socket.
pub struct TcpTransport {
    /// Local address.
    bind_addr: SocketAddr,
    /// Connected remote address.
    remote_addr: SocketAddr,
    /// The tcp writer.
    write_half: Mutex<WriteHalf<TcpStream>>,
}

impl TcpTransport {
    pub(crate) async fn connect<A>(addr: A, transports: &TransportLayer) -> Result<Transport>
    where
        A: ToSocketAddrs + Send,
    {
        let stream = TcpStream::connect(addr).await?;

        let bind_addr = stream.local_addr()?;
        let remote_addr = stream.peer_addr()?;

        let (read, write) = split(stream);
        let decoder = StreamingDecoder::new();

        let read_half = FramedRead::new(read, decoder);
        let write_half = Mutex::new(write);

        let transport = Transport::new(TcpTransport {
            bind_addr,
            remote_addr,
            write_half,
        });

        // TODO: Start keep-alive timer.
        transports.register_transport(transport.key(), transport.clone());

        let transports = transports.clone();
        let tcp = transport.clone();
        tokio::spawn(async move {
            if let Err(err) = tcp_read(read_half, remote_addr, tcp, transports).await {
                log::warn!("An error occured; error = {:#}", err);
            }
        });

        Ok(transport)
    }
}

#[async_trait]
impl SipTransport for TcpTransport {
    async fn send_msg(&self, data: &[u8], _dest: &SocketAddr) -> Result<usize> {
        let mut mguard = self.write_half.lock().await;

        mguard.write_all(data).await?;
        mguard.flush().await?;

        drop(mguard);

        Ok(data.len())
    }

    fn remote_addr(&self) -> Option<SocketAddr> {
        Some(self.remote_addr)
    }

    fn protocol(&self) -> TransportProtocol {
        TransportProtocol::Tcp
    }

    fn local_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    fn is_reliable(&self) -> bool {
        true
    }

    fn is_secure(&self) -> bool {
        false
    }
}

/// A TCP server socket that listens for incoming SIP connections.
///
/// The [`TcpListener`] accepts new TCP connections and spawns a dedicated
/// task for each one. Each accepted connection is wrapped into a [`TcpTransport`]
/// and registered into the [`Endpoint`].
pub struct TcpListener {
    /// Listener for TCP sockets.
    listener: TokioTcpListener,
    /// The local listener address.
    addr: SocketAddr,
}

impl TcpListener {
    /// Creates a new `TcpListener`, which will be bound to the specified address.
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> Result<TcpListener> {
        let listener = TokioTcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        Ok(Self { listener, addr })
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Accepts incoming TCP connections and handles them asynchronously.
    pub async fn accept_clients(self, transports: TransportLayer) -> Result<()> {
        while let Ok((stream, addr)) = self.listener.accept().await {
            log::debug!("Got incoming TCP connection from {}", addr);
            // Spawn a new task to handle the connection.
            tokio::spawn(Self::on_accepted((stream, addr), transports.clone()));
        }
        Ok(())
    }

    async fn on_accepted(
        (stream, addr): (TcpStream, SocketAddr),
        transports: TransportLayer,
    ) -> Result<()> {
        let bind_addr = stream.local_addr()?;
        let remote_addr = stream.peer_addr()?;

        let (read, write) = split(stream);
        let decoder = StreamingDecoder::new();

        let read_half = FramedRead::new(read, decoder);
        let write_half = Mutex::new(write);

        let transport = Transport::new(TcpTransport {
            bind_addr,
            remote_addr,
            write_half,
        });
        transports.register_transport(transport.key(), transport.clone());

        if let Err(err) = tcp_read(read_half, addr, transport, transports).await {
            log::warn!("An error occured; error = {:#}", err);
        }

        Ok(())
    }
}

async fn tcp_read(
    mut framed: FramedRead<ReadHalf<TcpStream>, StreamingDecoder>,
    peer: SocketAddr,
    transport: Transport,
    transports: TransportLayer,
) -> Result<()> {
    loop {
        match framed.next().await {
            Some(Ok(FramedMessage::Complete(data))) => {
                let packet = Packet::new(data, peer);
                let transport = transport.clone();
                let msg = TransportMessage { transport, packet };

                transports.receive_message(msg);
            }
            Some(Ok(FramedMessage::KeepaliveRequest)) => {
                transport.send_msg(KEEPALIVE_RESPONSE, &peer).await?;
            }
            Some(Ok(FramedMessage::KeepaliveResponse)) => (),
            Some(Err(err)) => {
                return Err(Error::Io(err));
            }
            None => {
                log::info!("TCP connection disconnected: {}", peer);
                transports.remove_transport(&transport.key());
                break;
            }
        };
    }

    Ok(())
}
