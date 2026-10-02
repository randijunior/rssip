pub mod packet;
pub mod header;

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