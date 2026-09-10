pub mod header;
pub mod packet;

use std::{io, net::SocketAddr};

use tokio::net::UdpSocket;

use crate::rtp::packet::RtpPacket;

pub struct RtpSession {
    // Local RTP address
    local_addr: SocketAddr,

    sock: UdpSocket,

    // ssrc: u32
}


impl RtpSession {
    pub async fn create(local_addr: SocketAddr) -> std::io::Result<Self> {
        let sock = UdpSocket::bind(local_addr).await?;

        todo!()
    }

    async fn rtp_recv(&self) -> io::Result<RtpPacket> {
        let mut buf = [0; 1024];
        let (len, addr) = self.sock.recv_from(&mut buf).await?;

        println!("{:?} bytes received from {:?}", len, addr);

        let packet = RtpPacket::parse(&buf[..len])?;

        Ok(packet)
    }
}
