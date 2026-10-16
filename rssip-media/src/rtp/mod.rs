pub mod header;
pub mod packet;

use std::io::{Result as IoResult, Write};
use std::net::SocketAddr;

use tokio::net::{ToSocketAddrs, UdpSocket};

use crate::codec::PayloadType;
use crate::rtp::header::RtpHeader;
use crate::rtp::packet::RtpPacket;

const MAX_RTP_PACKET_SIZE: usize = 1500;

const RTP_VERSION: u8 = 2;

pub struct RtpSession {
    /// Local RTP address
    local_addr: SocketAddr,

    /// Remote RTP address
    remote_addr: SocketAddr,

    sock: UdpSocket,

    ssrc: u32,

    peer_ssrc: Option<u32>,

    sequence_number: u16,

    timestamp: u32,
    
    payload_type: PayloadType,
    // cname: String,
    // rtcp_sock: UdpSocket
}

impl RtpSession {
    pub async fn create<A: ToSocketAddrs>(
        local_addr: A,
        remote_addr: A,
        payload_type: PayloadType,
    ) -> IoResult<Self> {
        let sock = UdpSocket::bind(local_addr).await?;

        sock.connect(remote_addr).await?;

        Ok(Self {
            local_addr: sock.local_addr()?,
            remote_addr: sock.peer_addr()?,
            sock,
            ssrc: rand::random(),
            sequence_number: rand::random(),
            timestamp: rand::random(),
            peer_ssrc: None,
            payload_type,
        })
    }

    pub async fn read_rtp(&self) -> IoResult<RtpPacket> {
        let mut buf = [0u8; MAX_RTP_PACKET_SIZE];

        let (len, _addr) = self.sock.recv_from(&mut buf).await?;

        let rtp_packet = RtpPacket::parse(&mut &buf[..len])?;

        Ok(rtp_packet)
    }

    pub async fn write_rtp(&mut self, data: &[u8], samples: u32) -> IoResult<usize> {
        self.timestamp = self.timestamp.wrapping_add(samples);
        self.sequence_number = self.sequence_number.wrapping_add(1);
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
        let len = self.send_rtp_packet(header, data).await?;
        Ok(len)
    }

    async fn send_rtp_packet(&self, header: RtpHeader, data: &[u8]) -> IoResult<usize> {
        use bytes::BufMut;
        let mut dst = [0u8; MAX_RTP_PACKET_SIZE];
        let mut buf = &mut dst[..];
        let remaining_before = buf.remaining_mut();
        let mut b = RTP_VERSION << 6;
        if header.padding {
            b |= 0b0010_0000;
        }
        if header.extension {
            b |= 0b0001_0000;
        }
        b |= header.csrc_list.len() as u8;
        buf.put_u8(b);

        let mut b = 0;
        if header.marker {
            b |= 0b1000_0000;
        }
        b |= header.payload_type;
        buf.put_u8(b);

        buf.put_u16(header.sequence_number);
        buf.put_u32(header.timestamp);
        buf.put_u32(header.ssrc);

        for csrc in header.csrc_list.iter() {
            buf.put_u32(*csrc);
        }
        //if let Some(ref extension) = self.extension {
        //
        // }
        buf.put(data);

        let remaining_after = buf.remaining_mut();
        let written_len = remaining_before - remaining_after;

        // Send a message
        let len = self.sock.send(&dst[..written_len]).await?;

        Ok(len)
    }
}
