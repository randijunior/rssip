//! UDP transport implementation for SIP.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::{ToSocketAddrs, UdpSocket};

use super::{Packet, SipTransport, Transport, TransportProtocol};
use crate::error::Result;
use crate::transport::{KEEPALIVE_REQUEST, KEEPALIVE_RESPONSE, TransportLayer, TransportMessage};

/// UDP transport implementation.
///
/// The [`UdpTransport`] provides a non-reliable, connectionless transport layer for
/// SIP messages. It wraps a [`UdpSocket`] and exposes methods for sending and
/// receiving datagrams.
///
/// This transport type is suitable for most SIP messages that do not require
/// reliability or retransmission.
#[derive(Debug, Clone)]
pub struct UdpTransport {
    inner: Arc<UdpInner>,
}

#[derive(Debug)]
struct UdpInner {
    sock: UdpSocket,
    addr: SocketAddr,
}

impl UdpTransport {
    /// Creates a new UDP transport addr to the specified address.
    ///
    /// This method binds an underlying [`UdpSocket`] to the given local address and
    /// returns a transport instance that can be used to send or receive SIP
    /// packets.
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let sock = UdpSocket::bind(addr).await?;
        let addr = sock.local_addr()?;
        Ok(Self {
            inner: Arc::new(UdpInner { sock, addr }),
        })
    }

    /// Receive UDP datagrams on this transport.
    pub(crate) async fn receive_datagram(self, transports: TransportLayer) -> Result<()> {
        let udp_tp = Transport::new(self.clone());
        // Buffer to recv packet.
        let mut buf = vec![0u8; 4000];
        loop {
            // Read data into buf.
            let (len, source) = self.inner.sock.recv_from(&mut buf).await?;
            if len == 0 {
                log::error!("[{}] Got an empty message from the peer.", source);
                continue;
            }

            let datagram = &buf[..len];

            if datagram == KEEPALIVE_REQUEST {
                self.inner
                    .sock
                    .send_to(KEEPALIVE_RESPONSE, source)
                    .await
                    .ok();
                continue;
            }

            if datagram == KEEPALIVE_RESPONSE {
                continue;
            }

            // Copy buf.
            let datagram_msg = bytes::Bytes::copy_from_slice(datagram);
            // Create Packet.
            let packet = Packet::new(datagram_msg, source);

            let msg = TransportMessage {
                transport: udp_tp.clone(),
                packet,
            };

            transports.receive_message(msg);
        }
    }
}

#[async_trait::async_trait]
impl SipTransport for UdpTransport {
    async fn send_msg(&self, buf: &[u8], addr: &SocketAddr) -> Result<usize> {
        Ok(self.inner.sock.send_to(buf, addr).await?)
    }

    fn remote_addr(&self) -> Option<SocketAddr> {
        None
    }

    fn protocol(&self) -> TransportProtocol {
        TransportProtocol::Udp
    }

    fn local_addr(&self) -> SocketAddr {
        self.inner.addr
    }

    fn is_reliable(&self) -> bool {
        false
    }

    fn is_secure(&self) -> bool {
        false
    }
}
