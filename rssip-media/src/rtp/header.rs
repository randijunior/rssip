pub const HEADER_LENGTH: usize = 4;

#[derive(Default, Clone)]
pub struct RtpHeader {
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub marker: bool,
    pub sequence_number: u16,
    pub payload_type: u8,
    pub ssrc: u32,
    pub csrc_list: Vec<u32>,
    pub timestamp: u32,
}

impl RtpHeader {
    pub fn parse(packet: &mut impl bytes::Buf) -> std::io::Result<Self> {
        let packet_len = packet.remaining();

        if packet_len < HEADER_LENGTH {
            todo!("return err")
        }
        let b0 = packet.get_u8();

        let version = b0 >> 6;
        let padding = (b0 & 0b0010_0000) != 0;
        let extension = (b0 & 0b0001_0000) != 0;
        let csrc_count = b0 & 0b0000_1111;

        let b1 = packet.get_u8();
        let marker = (b1 & 0b1000_0000) != 0;
        let payload_type = b1 & 0b0111_1111;

        let sequence_number = packet.get_u16();
        let timestamp = packet.get_u32();
        let ssrc = packet.get_u32();

        let csrc_list = (0..csrc_count).map(|_| packet.get_u32()).collect();

        Ok(Self {
            version,
            padding,
            extension,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            csrc_list,
        })
    }
}
