use std::io;

use crate::rtcp::header::RtcpHeader;

pub const RTCP_PACKET_TYPE_SR: u8 = 200;
pub const RTCP_PACKET_TYPE_RR: u8 = 201;
pub const RTCP_PACKET_TYPE_SDES: u8 = 202;
pub const RTCP_PACKET_TYPE_BYE: u8 = 203;
pub const RTCP_PACKET_TYPE_APP: u8 = 204;

pub const SDES_ITEM_TYPE_END: u8 = 0;
pub const SDES_ITEM_TYPE_CNAME: u8 = 1;
pub const SDES_ITEM_TYPE_NAME: u8 = 2;
pub const SDES_ITEM_TYPE_EMAIL: u8 = 3;
pub const SDES_ITEM_TYPE_PHONE: u8 = 4;
pub const SDES_ITEM_TYPE_LOC: u8 = 5;
pub const SDES_ITEM_TYPE_TOOL: u8 = 6;
pub const SDES_ITEM_TYPE_NOTE: u8 = 7;
pub const SDES_ITEM_TYPE_PRIV: u8 = 8;

const RECEPTION_REPORT_LENGTH: usize = 24;

fn read_bytes(raw_packet: &mut impl bytes::Buf, len: usize) -> io::Result<Vec<u8>> {
    let mut vec = vec![0; len];
    raw_packet.try_copy_to_slice(&mut vec)?;
    Ok(vec)
}

fn read_string(raw_packet: &mut impl bytes::Buf, len: usize) -> io::Result<String> {
    let bytes = read_bytes(raw_packet, len)?;
    String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

trait ParseRtcp: Sized {
    fn parse_rtcp(buf: &mut impl bytes::Buf, header: &RtcpHeader) -> io::Result<Self>;
}

// One RTCP raw_packet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtcpPacket {
    pub header: RtcpHeader,
    pub packet_kind: RtcpPacketKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtcpPacketKind {
    SR(SenderReport),
    RR(ReceiverReport),
    SDES(SourceDescription),
    BYE(GoodBye),
    APP(ApplicationDefined),
}

// SR: Sender Report RTCP Packet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderReport {
    pub ssrc: u32,
    pub ntp_timestamp: u64,
    pub rtp_timestamp: u32,
    pub packet_count: u32,
    pub octet_count: u32,
    pub reception_reports: Vec<ReceptionReport>,
    pub extensions: Vec<u8>,
}

// RR: Receiver Report RTCP Packet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverReport {
    pub ssrc: u32,
    pub reception_reports: Vec<ReceptionReport>,
    pub extensions: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceptionReport {
    pub ssrc: u32,
    pub fraction_lost: u8,
    pub packets_lost: i32,
    pub last_seq: u32,
    pub jitter: u32,
    pub last_sr_packet: u32,
    pub delay_since_last_sr: u32,
}

// SDES: Source Description RTCP Packet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDescription {
    pub chunks: Vec<SourceDescriptionChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDescriptionChunk {
    pub src: u32,                  /* first SSRC/CSRC */
    pub sdes_items: Vec<SdesItem>, /* list of SDES items */
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdesItem {
    Cname(String),
    Name(String),
    Email(String),
    Phone(String),
    Loc(String),
    Tool(String),
    Note(String),
    Priv(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoodBye {
    pub src_list: Vec<u32>, /* list of sources */
    pub reason_for_leaving: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationDefined {
    pub src: u32, // SSRC/CSRC
    pub name: [u8; 4],
    pub data: Vec<u8>,
    pub sub_type: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtcpCompoundPacket(Vec<RtcpPacket>);

impl RtcpPacket {
    pub fn parse(raw_packet: &mut impl bytes::Buf) -> io::Result<Self> {
        let header = RtcpHeader::parse(raw_packet)?;
        let packet_kind = match header.packet_type {
            RTCP_PACKET_TYPE_SR => {
                RtcpPacketKind::SR(SenderReport::parse_rtcp(raw_packet, &header)?)
            }
            RTCP_PACKET_TYPE_RR => {
                RtcpPacketKind::RR(ReceiverReport::parse_rtcp(raw_packet, &header)?)
            }
            RTCP_PACKET_TYPE_SDES => {
                RtcpPacketKind::SDES(SourceDescription::parse_rtcp(raw_packet, &header)?)
            }
            RTCP_PACKET_TYPE_BYE => RtcpPacketKind::BYE(GoodBye::parse_rtcp(raw_packet, &header)?),
            RTCP_PACKET_TYPE_APP => {
                RtcpPacketKind::APP(ApplicationDefined::parse_rtcp(raw_packet, &header)?)
            }
            kind => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!("Unknown rtcp packet kind: {}", kind),
                ));
            }
        };
        Ok(Self {
            header,
            packet_kind,
        })
    }
}

impl RtcpCompoundPacket {
    pub fn parse(buf: &mut impl bytes::Buf) -> io::Result<Self> {
        let mut packets = Vec::new();

        while buf.has_remaining() {
            packets.push(RtcpPacket::parse(buf)?);
        }

        Ok(Self(packets))
    }
    pub fn packets(&self) -> &[RtcpPacket] {
        &self.0
    }

    pub fn packets_mut(&mut self) -> &mut [RtcpPacket] {
        &mut self.0
    }

    pub fn into_packets(self) -> Vec<RtcpPacket> {
        self.0
    }
}

impl ParseRtcp for SenderReport {
    fn parse_rtcp(buf: &mut impl bytes::Buf, header: &RtcpHeader) -> io::Result<SenderReport> {
        let ssrc = buf.try_get_u32()?;
        let ntp_timestamp = buf.try_get_u64()?;
        let rtp_timestamp = buf.try_get_u32()?;
        let packet_count = buf.try_get_u32()?;
        let octet_count = buf.try_get_u32()?;

        let mut reception_reports = Vec::with_capacity(header.count as usize);
        for _ in 0..header.count {
            reception_reports.push(ReceptionReport::parse(buf)?);
        }
        let extensions = read_bytes(buf, buf.remaining())?;

        Ok(SenderReport {
            ssrc,
            ntp_timestamp,
            rtp_timestamp,
            packet_count,
            octet_count,
            reception_reports,
            extensions,
        })
    }
}

impl ReceptionReport {
    pub fn parse(raw_packet: &mut impl bytes::Buf) -> io::Result<Self> {
        if raw_packet.remaining() < RECEPTION_REPORT_LENGTH {
            todo!("return err");
        }
        let ssrc = raw_packet.get_u32();
        let fraction_lost = raw_packet.get_u8();

        let lost = (u32::from(raw_packet.get_u8()) << 16)
            | (u32::from(raw_packet.get_u8()) << 8)
            | u32::from(raw_packet.get_u8());
        let packets_lost = ((lost << 8) as i32) >> 8;

        let last_seq = raw_packet.get_u32();
        let jitter = raw_packet.get_u32();
        let last_sr_packet = raw_packet.get_u32();
        let delay_since_last_sr = raw_packet.get_u32();

        Ok(ReceptionReport {
            ssrc,
            fraction_lost,
            packets_lost,
            last_seq,
            jitter,
            last_sr_packet,
            delay_since_last_sr,
        })
    }
}

impl ParseRtcp for ReceiverReport {
    fn parse_rtcp(buf: &mut impl bytes::Buf, header: &RtcpHeader) -> io::Result<ReceiverReport> {
        let ssrc = buf.try_get_u32()?;

        let mut reception_reports = Vec::with_capacity(header.count as usize);
        for _ in 0..header.count {
            reception_reports.push(ReceptionReport::parse(buf)?);
        }
        let extensions = read_bytes(buf, buf.remaining())?;

        Ok(ReceiverReport {
            ssrc,
            reception_reports,
            extensions,
        })
    }
}

impl ParseRtcp for SourceDescription {
    fn parse_rtcp(buf: &mut impl bytes::Buf, header: &RtcpHeader) -> io::Result<SourceDescription> {
        let mut chunks = vec![];
        for _ in 0..header.count {
            chunks.push(SourceDescriptionChunk::parse(buf)?);
        }
        Ok(SourceDescription { chunks })
    }
}

impl SourceDescriptionChunk {
    pub fn parse(raw_packet: &mut impl bytes::Buf) -> io::Result<Self> {
        let source = raw_packet.try_get_u32()?;

        let mut items = vec![];
        loop {
            let item_type = raw_packet.try_get_u8()?;

            if item_type == SDES_ITEM_TYPE_END {
                break;
            }
            let length = raw_packet.try_get_u8()?.into();
            let text = read_string(raw_packet, length)?;

            let item = match item_type {
                SDES_ITEM_TYPE_CNAME => SdesItem::Cname(text),
                SDES_ITEM_TYPE_NAME => SdesItem::Name(text),
                SDES_ITEM_TYPE_EMAIL => SdesItem::Email(text),
                SDES_ITEM_TYPE_PHONE => SdesItem::Phone(text),
                SDES_ITEM_TYPE_LOC => SdesItem::Loc(text),
                SDES_ITEM_TYPE_TOOL => SdesItem::Tool(text),
                SDES_ITEM_TYPE_NOTE => SdesItem::Note(text),
                SDES_ITEM_TYPE_PRIV => SdesItem::Priv(text),
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        format!("Unknown SDES item type: {}", item_type),
                    ));
                }
            };
            items.push(item);
            continue;
        }
        Ok(SourceDescriptionChunk {
            src: source,
            sdes_items: items,
        })
    }
}

impl ParseRtcp for GoodBye {
    fn parse_rtcp(buf: &mut impl bytes::Buf, header: &RtcpHeader) -> io::Result<GoodBye> {
        let ssrc_or_csrc_list = (0..header.count).map(|_| buf.get_u32()).collect();
        let reason = if let Ok(len) = buf.try_get_u8() {
            Some(read_string(buf, len.into())?)
        } else {
            None
        };
        Ok(GoodBye {
            src_list: ssrc_or_csrc_list,
            reason_for_leaving: reason,
        })
    }
}

impl ParseRtcp for ApplicationDefined {
    fn parse_rtcp(
        buf: &mut impl bytes::Buf,
        header: &RtcpHeader,
    ) -> io::Result<ApplicationDefined> {
        let src = buf.try_get_u32()?;
        let mut name = [0; 4];
        buf.try_copy_to_slice(&mut name)?;

        let data = read_bytes(buf, buf.remaining())?;

        Ok(ApplicationDefined {
            src,
            name,
            data,
            sub_type: header.count,
        })
    }
}
