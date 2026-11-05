pub mod header;
pub mod packet;

use bytes::BufMut;
use std::io::{self, ErrorKind, Result as IoResult};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, watch};

use tokio::net::{ToSocketAddrs, UdpSocket};

use crate::rtcp::packet::RtcpPacket;
use crate::rtp::header::RtpHeader;
use crate::rtp::packet::RtpPacket;

const MAX_RTP_PACKET_SIZE: usize = 1500;
const RTP_HEADER_SIZE: usize = 12;
const RTP_VERSION: u8 = 2;
const MAX_CSRC: usize = 15;

pub struct RtpSession {
    /// Local RTP address
    local_addr: SocketAddr,

    /// Remote RTP address
    remote_addr: SocketAddr,

    rtp_sock: Arc<UdpSocket>,
    rtcp_sock: Arc<UdpSocket>,

    receiver: mpsc::Receiver<RtpMessage>,

    _shutdown: watch::Sender<()>,

    ssrc: u32,

    peer_ssrc: Option<u32>,

    sequence_number: u16,

    timestamp: u32,

    payload_type: u8,
    // cname: String,
}

pub enum RtpMessage {
    Rtp(RtpPacket),
    Rtcp(RtcpPacket),
}

impl RtpSession {
    pub async fn create<L, R>(local_addr: L, remote_addr: R, payload_type: u8) -> IoResult<Self>
    where
        L: ToSocketAddrs,
        R: ToSocketAddrs,
    {
        if payload_type > 127 {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "payload_type sould be <= 127",
            ));
        }

        let rtp_sock = UdpSocket::bind(local_addr).await?;
        rtp_sock.connect(remote_addr).await?;
        let local_addr = rtp_sock.local_addr()?;
        let remote_addr = rtp_sock.peer_addr()?;

        let rtcp_port = |p: u16| {
            p.checked_add(1)
                .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "invalid RTCP port"))
        };
        let rtcp_sock = UdpSocket::bind((local_addr.ip(), rtcp_port(local_addr.port())?)).await?;
        rtcp_sock
            .connect((remote_addr.ip(), rtcp_port(remote_addr.port())?))
            .await?;
        let rtp_sock = Arc::new(rtp_sock);
        let rtcp_sock = Arc::new(rtcp_sock);
        let (tx, rx) = mpsc::channel(64);
        let (shutdown_tx, shutdown_rx) = watch::channel(());

        spawn_reader(
            "RTP",
            rtp_sock.clone(),
            tx.clone(),
            shutdown_rx.clone(),
            |mut b| {
                RtpPacket::parse(&mut b)
                    .map(RtpMessage::Rtp)
                    .map_err(|e| log::error!("RTP parse: {e:#?}"))
                    .ok()
            },
        );
        spawn_reader("RTCP", rtcp_sock.clone(), tx, shutdown_rx, |mut b| {
            RtcpPacket::parse(&mut b)
                .map(RtpMessage::Rtcp)
                .map_err(|e| log::error!("RTCP parse: {e:#?}"))
                .ok()
        });
        Ok(RtpSession {
            local_addr,
            remote_addr,
            rtp_sock,
            rtcp_sock,
            receiver: rx,
            ssrc: rand::random(),
            _shutdown: shutdown_tx,
            sequence_number: rand::random(),
            timestamp: rand::random(),
            peer_ssrc: None,
            payload_type,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
    pub fn peer_ssrc(&self) -> Option<u32> {
        self.peer_ssrc
    }

    pub async fn receive(&mut self) -> Option<RtpMessage> {
        let msg = self.receiver.recv().await;
        if let Some(RtpMessage::Rtp(rtp)) = &msg {
            self.peer_ssrc.get_or_insert(rtp.header.ssrc);
        }
        msg
    }

    pub async fn write_rtp(&mut self, data: &[u8], samples: u32) -> IoResult<usize> {
        let header = RtpHeader {
            version: RTP_VERSION,
            padding: false,
            extension: false,
            marker: false,
            sequence_number: self.sequence_number,
            payload_type: self.payload_type,
            ssrc: self.ssrc,
            csrc_list: vec![],
            timestamp: self.timestamp,
        };
        let written = self.send_rtp_packet(header, data).await?;
        self.timestamp = self.timestamp.wrapping_add(samples);
        self.sequence_number = self.sequence_number.wrapping_add(1);
        Ok(written)
    }

    pub async fn send_rtp_packet(&self, header: RtpHeader, data: &[u8]) -> IoResult<usize> {
        let total_size = RTP_HEADER_SIZE + header.csrc_list.len() * 4 + data.len();
        if total_size > MAX_RTP_PACKET_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "RTP packet exceeds maximum size",
            ));
        }
        let mut dst = [0u8; MAX_RTP_PACKET_SIZE];
        let mut buf = &mut dst[..];
        buf.put_u8((RTP_VERSION << 6) | header.csrc_list.len() as u8);
        buf.put_u8(((header.marker as u8) << 7) | header.payload_type);
        buf.put_u16(header.sequence_number);
        buf.put_u32(header.timestamp);
        buf.put_u32(header.ssrc);
        for csrc in &header.csrc_list {
            buf.put_u32(*csrc);
        }
        buf.put_slice(data);

        self.rtp_sock.send(&dst[..total_size]).await
    }
}

fn spawn_reader<F>(
    name: &'static str,
    sock: Arc<UdpSocket>,
    tx: mpsc::Sender<RtpMessage>,
    mut shutdown: watch::Receiver<()>,
    parse: F,
) where
    F: Fn(&[u8]) -> Option<RtpMessage> + Send + 'static,
{
    tokio::spawn(async move {
        let mut buf = [0u8; MAX_RTP_PACKET_SIZE];
        loop {
            let len = tokio::select! {
                _ = shutdown.changed() => {
                    log::debug!("Cancelling {name} task...");
                    break;
                }
                res = sock.recv(&mut buf) => match res {
                    Ok(len) => len,
                    Err(e) if e.kind() == ErrorKind::ConnectionRefused => continue,
                    Err(e) => {
                        log::error!("{name} socket error: {e}");
                        break;
                    }
                },
            };

            let Some(msg) = parse(&buf[..len]) else {
                continue;
            };

            match tx.try_send(msg) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    log::warn!("{name}: slow consumer, discarded packet")
                }
                Err(TrySendError::Closed(_)) => break,
            }
        }
    });
}
