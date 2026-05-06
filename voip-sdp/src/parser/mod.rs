use utils::scanner::{is_newline, is_space};
use utils::{LookupTable, Scanner, lookup_table};

use crate::error::{ParseSdpError, Result};
use crate::msg::*;

type SdpField = u8;

// Attributes
const PROTOCOL_VERSION: SdpField = b'v';
const ORIGIN: SdpField = b'o';
const SESSION_NAME: SdpField = b's';
const SESSION_INFORMATION: SdpField = b'i';
const URI: SdpField = b'u';
const EMAIL_ADDRESS: SdpField = b'e';
const PHONE_NUMBER: SdpField = b'p';
const CONNECTION_INFORMATION: SdpField = b'c';
const BANDWIDTH_INFORMATION: SdpField = b'b';
const TIME_ACTIVE: SdpField = b't';
const REPEAT_TIME: SdpField = b'r';
const TIME_ZONE_OFFSET: SdpField = b'z';
const MEDIA_DESCRIPTION: SdpField = b'm';
const ATTRIBUTE: SdpField = b'a';

const TOKEN: &str = "!#$%&'*+-.^_`{|}~";
const ALPHANUMERIC: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

const TOKEN_TAB: LookupTable = lookup_table!(ALPHANUMERIC, TOKEN);

/// A SDP message parser.
pub struct SdpParser<'buf> {
    scanner: Scanner<'buf>,
}

impl<'buf> SdpParser<'buf> {
    /// Construct a new `SdpParser`.
    #[inline]
    pub fn new(buf: &'buf (impl AsRef<[u8]> + ?Sized)) -> Self {
        Self {
            scanner: Scanner::new(buf.as_ref()),
        }
    }

    #[inline]
    pub fn parse(buf: &'buf (impl AsRef<[u8]> + ?Sized)) -> Result<SessionDescription> {
        Self::new(buf).parse_sdp()
    }

    /// Parses the `buf` into a [`SessionDescription`].
    pub fn parse_sdp(&mut self) -> Result<SessionDescription> {
        let mut sdp = SessionDescription::default();

        while !self.is_eof() {
            let sdp_field = self.read_field()?;

            match sdp_field {
                ATTRIBUTE => {
                    let attr = self.parse_attribute()?;
                    sdp.set_attr(attr);
                }
                PROTOCOL_VERSION => {
                    self.parse_version()?;
                }
                ORIGIN => {
                    let origin = self.parse_origin()?;
                    sdp.set_origin(origin);
                }
                SESSION_NAME => {
                    let name = self.read_line()?;
                    sdp.set_name(name.to_owned());
                }
                CONNECTION_INFORMATION => {
                    let conn = self.parse_connection_info()?;
                    sdp.set_connection(conn);
                }
                SESSION_INFORMATION => {
                    let name = self.read_line()?;
                    sdp.set_information(name.to_owned());
                }

                URI => {
                    let uri = self.read_line()?;
                    sdp.set_uri(uri.to_owned());
                }
                EMAIL_ADDRESS => {
                    let email = self.read_line()?;
                    sdp.set_email_addr(email.to_owned());
                }
                PHONE_NUMBER => {
                    let phone = self.read_line()?;
                    sdp.set_phone(phone.to_owned());
                }
                BANDWIDTH_INFORMATION => {
                    let bandwidth = self.parse_bandwidth()?;
                    sdp.set_bandwidth_information(bandwidth);
                }

                TIME_ACTIVE => {
                    let timing = self.parse_time()?;
                    sdp.set_time_desc(timing);
                }
                REPEAT_TIME => {
                    let repeat = self.parse_repeat_times()?;
                    sdp.set_repeat_times(repeat)?;
                }
                TIME_ZONE_OFFSET => {}

                MEDIA_DESCRIPTION => {
                    let media = self.parse_media_description()?;
                    sdp.set_media_description(media);
                }
                _ => (),
            }

            self.handle_new_line();
        }

        Ok(sdp)
    }

    #[inline]
    fn handle_new_line(&mut self) {
        let _ = self.scanner.scan_while(is_newline);
    }

    fn parse_version(&mut self) -> Result<()> {
        self.scanner
            .must_read(b'0')
            .map_err(|_| ParseSdpError::SdpInvalidProtocolVersion)?;

        Ok(())
    }

    #[inline]
    fn handle_ws(&mut self) {
        let _ = self.scanner.scan_while(is_space);
    }

    fn parse_origin(&mut self) -> Result<Origin> {
        let user = self
            .scanner
            .scan_while_as_str(|b| !b.is_ascii_whitespace())?
            .to_owned();
        self.handle_ws();
        let session_id = self.scanner.scan_u64()?;
        self.handle_ws();
        let session_version = self.scanner.scan_u64()?;
        self.handle_ws();
        let nettype = self.scanner.scan_until_as_str(b' ')?.to_owned();
        self.handle_ws();
        let addrtype = self.scanner.scan_until_as_str(b' ')?.to_owned();
        self.handle_ws();
        let unicast_address = self.scanner.scan_until_any_as_str(b" \t\r\n")?.to_owned();
        self.handle_ws();

        Ok(Origin {
            user,
            session_id,
            session_version,
            nettype,
            addrtype,
            unicast_address,
        })
    }

    fn parse_connection_info(&mut self) -> Result<ConnectionInformation> {
        let nettype = if self.scanner.matches_prefix(b"IN") {
            self.scanner.advance_n(2);
            NetType::IN
        } else {
            let other_type = self.scanner.scan_while_as_str(|b| !is_space(b))?;
            NetType::Other(other_type.to_owned())
        };

        self.handle_ws();

        let addrtype = match self.scanner.peek_n(3) {
            Some(b"IP4") => AddrType::IP4,
            Some(b"IP6") => AddrType::IP6,
            _ => {
                return Err(ParseSdpError::SyntaxError {
                    s: "Invalid Addr Type".to_owned(),
                }
                .into());
            }
        };
        self.scanner.advance_n(3);

        self.handle_ws();
        let conection_address = self.read_line()?.to_owned();
        self.handle_ws();

        Ok(ConnectionInformation {
            nettype,
            addrtype,
            conection_address,
        })
    }

    fn parse_media_description(&mut self) -> Result<MediaDescription> {
        let media_type = match self.read_token() {
            "audio" => MediaType::Audio,
            "video" => MediaType::Video,
            "text" => MediaType::Text,
            "message" => MediaType::Message,
            _ => return Err(ParseSdpError::SdpUnknowMediaType.into()),
        };
        self.handle_ws();
        let port = self.scanner.scan_u16()?;
        self.handle_ws();

        let bytes = self.scanner.scan_while(|b| !is_space(b));

        let proto = match bytes {
            b"UDP" | b"udp" => SdpTransport::UDP,
            b"RTP/AVP" => SdpTransport::RTPAVP,
            b"RTP/AVPF" => SdpTransport::RTPAVPF,
            b"RTP/SAVP" => SdpTransport::RTPSAVP,
            b"RTP/SAVPF" => SdpTransport::RTPSAVPF,
            _ => {
                return Err(ParseSdpError::SdpUnknowTransport.into());
            }
        };

        let mut media_formats = vec![];

        while self.scanner.scan_if(is_space).is_some() {
            let fmt = self.read_token();

            media_formats.push(fmt.to_owned());

            if matches!(self.scanner.peek(), Some(b'\r') | Some(b'\n') | None) {
                break;
            }
        }

        Ok(MediaDescription {
            media_type,
            port,
            number_of_ports: None,
            proto,
            media_formats,
            title: None,
            connection_information: None,
            bandwidth_information: vec![],
            attributes: vec![],
        })
    }

    fn parse_time(&mut self) -> Result<TimeDescription> {
        let start_time = self.scanner.scan_u64()?;

        self.scanner.must_read(b' ')?;

        let stop_time = self.scanner.scan_u64()?;

        Ok(TimeDescription {
            time_active: TimeActive {
                start_time,
                stop_time,
            },
            repeat_times: vec![],
        })
    }

    fn parse_repeat_times(&mut self) -> Result<RepeatTimes> {
        let repeat_interval = self.scanner.scan_i64()?;
        self.handle_ws();
        let active_duration = self.scanner.scan_i64()?;

        let mut offsets = vec![];

        while self.scanner.scan_if(is_space).is_some() {
            let offset = self.scanner.scan_i64()?;
            offsets.push(offset);

            if matches!(self.scanner.peek(), Some(b'\r') | Some(b'\n') | None) {
                break;
            }
        }

        Ok(RepeatTimes {
            repeat_interval,
            active_duration,
            offsets,
        })
    }

    fn parse_bandwidth(&mut self) -> Result<BandwidthInformation> {
        let bwtype = match self.scanner.scan_until_as_str(b':')? {
            "CT" => Bwtype::CT,
            "AS" => Bwtype::AS,
            "RR" => Bwtype::RR,
            "RS" => Bwtype::RS,
            "TIAS" => Bwtype::TIAS,
            other => Bwtype::Other(other.to_owned()),
        };
        let bandwidth = self.scanner.scan_u64()?;

        Ok(BandwidthInformation { bwtype, bandwidth })
    }

    fn parse_attribute(&mut self) -> Result<Attribute> {
        let attr_name = self.read_token().to_owned();

        let attr_value = if self.scanner.scan_if_eq(b':').is_some() {
            Some(self.read_line()?.to_owned())
        } else {
            None
        };

        Ok(Attribute {
            name: attr_name,
            value: attr_value,
        })
    }

    pub fn parse_rtpmap(&mut self) -> Result<RtpMap> {
        // a=rtpmap:98 L16/16000/2.
        let payload_type = self.read_token().to_owned();
        self.handle_ws();
        let encoding_name = self.read_token().to_owned();

        self.scanner.must_read(b'/')?;

        let clock_rate = self.scanner.scan_u32()?;

        let param = if self.scanner.scan_if_eq(b'/').is_some() {
            Some(self.read_token().to_owned())
        } else {
            None
        };

        self.handle_new_line();

        Ok(RtpMap {
            payload_type,
            enc_name: encoding_name,
            clock_rate,
            param,
        })
    }

    #[inline]
    fn read_token(&mut self) -> &str {
        unsafe { self.scanner.scan_while_as_str_unchecked(is_token) }
    }

    #[inline]
    fn read_line(&mut self) -> Result<&str> {
        let attr = self.scanner.scan_line()?;
        Ok(attr)
    }

    fn read_field(&mut self) -> Result<SdpField> {
        let field = self.scanner.next_byte()?;

        self.scanner.must_read(b'=')?;

        Ok(field)
    }

    fn is_eof(&self) -> bool {
        self.scanner.is_eof()
    }
}

#[must_use]
pub(crate) fn is_token(b: u8) -> bool {
    TOKEN_TAB[b as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_example_sdp() {
        let example_sdp = concat! {
            "v=0\r\n",
            "o=jdoe 3724394400 3724394405 IN IP4 198.51.100.1\r\n",
            "s=Call to John Smith\r\n",
            "i=SDP Offer #1\r\n",
            "u=http://www.jdoe.example.com/home.html\r\n",
            "e=Jane Doe <jane@jdoe.example.com>\r\n",
            "p=+1 617 555-6011\r\n",
            "c=IN IP4 198.51.100.1\r\n",
            "t=0 0\r\n",
            "m=audio 49170 RTP/AVP 0\r\n",
            "m=audio 49180 RTP/AVP 0\r\n",
            "m=video 51372 RTP/AVP 99\r\n",
            "c=IN IP6 2001:db8::2\r\n",
            "a=rtpmap:99 h263-1998/90000\r\n"
        };

        SdpParser::parse(example_sdp).unwrap();
    }

    #[test]
    fn test_parse_sdp() {
        let sdp = concat! {
            "v=0\r\n",
            "o=10009e 3642 1263 IN IP4 127.0.0.1  \r\n",
            "s=Talk\r\n",
            "c=IN IP4 127.0.0.1\r\n",
            "t=0 0\r\n",
            "a=ice-pwd:d426d2d77a920581717011cb\r\n",
            "a=ice-ufrag:0bf83d88\r\n",
            "a=rtcp-xr:rcvr-rtt=all:10000 stat-summary=loss,dup,jitt,TTL voip-metrics\r\n",
            "m=audio 48616 RTP/AVPF 96 97 98 0 8 3 9 99 10 11 101 100 102 103 104\r\n",
            "c=IN IP4 168.181.99.178\r\n",
            "a=rtpmap:96 opus/48000/2\r\n",
            "a=fmtp:96 useinbandfec=1\r\n",
            "a=rtpmap:97 speex/16000\r\n",
            "a=fmtp:97 vbr=on\r\n",
            "a=rtpmap:98 speex/8000\r\n",
            "a=fmtp:98 vbr=on\r\n",
            "a=rtpmap:99 speex/32000\r\n",
            "a=fmtp:99 vbr=on\r\n",
            "a=rtpmap:101 telephone-event/48000\r\n",
            "a=rtpmap:100 telephone-event/16000\r\n",
            "a=rtpmap:102 telephone-event/8000\r\n",
            "a=rtpmap:103 telephone-event/32000\r\n",
            "a=rtpmap:104 telephone-event/44100\r\n",
            "a=candidate:1 1 UDP 2130706303 192.168.0.115 57881 typ host\r\n",
            "a=candidate:1 2 UDP 2130706302 192.168.0.115 48948 typ host\r\n",
            "a=candidate:2 1 UDP 2130706303 100.65.0.2 57881 typ host\r\n",
            "a=candidate:2 2 UDP 2130706302 100.65.0.2 48948 typ host\r\n",
            "a=candidate:3 1 UDP 1694498687 168.181.99.178 48616 typ srflx raddr 192.168.0.115 rport 57881\r\n",
            "a=candidate:3 2 UDP 1694498686 168.181.99.178 48617 typ srflx raddr 192.168.0.115 rport 48948\r\n",
            "a=rtcp-fb:* trr-int 1000\r\n",
            "a=rtcp-fb:* ccm tmmbr\r\n",
        };

        let sdp = SdpParser::parse(sdp).unwrap();
    }
}
