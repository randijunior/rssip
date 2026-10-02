use crate::rtcp::header::RtcpHeaderCommon;

// One RTCP packet
pub struct RtcpPacket {
    header: RtcpHeaderCommon,
    packet_kind: RtcpPacketKind
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtcpPacketKind {
    SR(SenderReport),
    RR(ReceiverReport),
    SDES(SourceDescription),
    BYE(GoodBye),
    APP(ApplicationDefined),
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderReport {
    pub ssrc: u32,
    pub padding: bool,
    pub ntp_timestamp: u64,
    pub rtp_timestamp: u32,
    pub sender_packet_count: u32,
    pub sender_octet_count: u32,
    pub reception_reports: Vec<ReceptionReport>,
    pub extensions: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverReport {
    pub ssrc: u32,
    pub reception_reports: Vec<ReceptionReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceptionReport {
    pub ssrc: u32,
    pub fraction_lost: u8,
    pub packets_lost: u32,
    pub last_seq: u32,
    pub jitter: u32,
    pub last_sr_packet: u32,
    pub delay_since_last_sr: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDescription {
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
    src_list: Vec<u32>, /* list of sources */
    reason_for_leaving: String
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationDefined {
    pub src: u32, // SSRC/CSRC
    pub name: [u8; 4],
    pub data: Vec<u8>,
    pub sub_type: u8
}
