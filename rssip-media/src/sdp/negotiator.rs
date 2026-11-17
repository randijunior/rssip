use std::net::IpAddr;

use crate::codec::Codec;
use crate::error::{Error, Result};
use crate::sdp::{
    AddrType, Attribute, ConnectionInformation, Direction, MediaDescription, MediaType, NetType,
    Origin, SdpTransport, SessionDescription, TimeActive, TimeDescription,
};

pub enum Role {
    Offerer,
    Answerer,
}

#[derive(Default)]
pub enum Negotiator {
    #[default]
    Idle,
    RemoteOffered {
        offer: SessionDescription,
    },
    LocalOffered {
        offer: SessionDescription,
    },
    Complete {
        offer: SessionDescription,
        answer: SessionDescription,
        role: Role,
    },
}

pub struct SdpConfig {
    origin_ip: IpAddr,
    direction: Direction,
    media_streams: Vec<SdpMediaStream>,
}

pub struct SdpConfigBuilder {
    address: IpAddr,
    direction: Direction,
    port: u16,
    codecs: Vec<Codec>,
    transport: SdpTransport,
}

pub struct SdpMediaStream {
    transport: SdpTransport,
    media_type: MediaType,
    codecs: Vec<Codec>,
    port: u16,
}

impl Negotiator {
    pub fn new() -> Self {
        Self::Idle
    }

    // RFC 3264 5 - Generating the Initial Offer
    pub fn generate_offer(&mut self, config: &SdpConfig) -> Result<&SessionDescription> {
        if !matches!(self, Self::Idle) {
            return Err(Error::ErrInvalidNegoState);
        }
        let offer = self.build_offer(config);

        *self = Negotiator::LocalOffered { offer };

        let Self::LocalOffered { offer } = &*self else {
            unreachable!()
        };

        Ok(offer)
    }

    // RFC 3264 6 - Generating the Answer
    pub fn generate_answer(&mut self, config: &SdpConfig) -> Result<&SessionDescription> {
        let Self::RemoteOffered { offer: remote } = &self else {
            return Err(Error::ErrInvalidNegoState);
        };

        let local = self.build_offer(config);

        let mut media: Vec<MediaDescription> = vec![];

        for (local, remote) in local.media.iter().zip(remote.media.iter()) {
            if local.media_type != remote.media_type {
                todo!("return err");
            }

            if local.proto != remote.proto {
                todo!("return err");
            }

            if remote.port == 0 || local.port == 0 {
                continue;
            }

            let local_dir = local
                .attributes
                .iter()
                .find(|attr| {
                    let name = attr.name.as_str();
                    matches!(name, "recvonly" | "sendrecv" | "sendonly" | "inactive")
                })
                .map(|attr| attr.name.as_str());

            let remote_dir = remote
                .attributes
                .iter()
                .find(|attr| {
                    let name = attr.name.as_str();
                    matches!(name, "recvonly" | "sendrecv" | "sendonly" | "inactive")
                })
                .map(|attr| attr.name.as_str());

            let answer_dir = match remote_dir {
                Some("sendonly") => match local_dir {
                    Some("sendrecv") | Some("recvonly") => Some("recvonly".to_owned()),
                    _ => Some("inactive".to_owned()),
                },
                Some("inactive") => Some("inactive".to_owned()),
                Some("recvonly") => match local_dir {
                    Some("sendrecv") | Some("sendonly") => Some("sendonly".to_owned()),
                    _ => Some("inactive".to_owned()),
                },
                Some("sendrecv") => Some("sendrecv".to_owned()),
                Some(_unknow) => todo!("return err"),
                None => None,
            };

            let attributes = if let Some(media_direction) = answer_dir {
                let mut media_attrs: Vec<_> = local
                    .attributes
                    .iter()
                    .filter(|attr| {
                        let name = attr.name.as_str();
                        !matches!(name, "recvonly" | "sendrecv" | "sendonly" | "inactive")
                    })
                    .map(ToOwned::to_owned)
                    .collect();

                media_attrs.push(Attribute {
                    name: media_direction,
                    value: None,
                });
                media_attrs
            } else {
                local.attributes.clone()
            };

            let mut media_formats = vec![];

            for media_format in &local.media_formats {
                let payload_type: u8 = media_format.parse::<u8>()?;

                if payload_type < 96 {
                    if remote.media_formats.contains(&media_format) {
                        media_formats.push(media_format.to_owned());
                    } else {
                        // ignoring no matched codec
                        continue;
                    }
                } else {
                    // TODO: dynamic payload type
                    unimplemented!("dynamic payload type");
                }
            }
            // TODO: if media format is empty return err?

            media.push(MediaDescription {
                media_formats,
                attributes,
                ..local.clone()
            });
        }

        let answer = SessionDescription {
            media,
            time: remote.time.clone(),
            ..local.clone()
        };

        let Self::RemoteOffered { offer } = std::mem::take(self) else {
            unreachable!()
        };

        *self = Negotiator::Complete {
            offer,
            answer,
            role: Role::Answerer,
        };

        let Self::Complete { answer, .. } = &*self else {
            unreachable!()
        };

        Ok(answer)
    }

    // RFC 3264 7 - Offerer Processing of the Answer
    pub fn receive_answer(&mut self, answer: SessionDescription) -> Result<()> {
        let Self::LocalOffered { offer } = self else {
            return Err(Error::ErrInvalidNegoState);
        };
        // TODO: Return accepted Streams?

        for (local, remote) in offer.media.iter().zip(answer.media.iter()) {
            // Just check if have matched codecs
            if local.media_type != remote.media_type {
                todo!("return err");
            }

            if local.proto != remote.proto {
                todo!("return err");
            }

            if remote.port == 0 || local.port == 0 {
                continue;
            }
        }

        let Self::LocalOffered { offer } = std::mem::take(self) else {
            unreachable!()
        };

        *self = Self::Complete {
            offer,
            answer,
            role: Role::Offerer,
        };

        Ok(())
    }

    fn build_offer(&self, config: &SdpConfig) -> SessionDescription {
        let mut media: Vec<MediaDescription> = config
            .media_streams
            .iter()
            .map(|sdp_media_stream| {
                let mut media_formats = vec![];
                let mut attributes = vec![];

                for codec in &sdp_media_stream.codecs {
                    match codec.name() {
                        "PCMU" => {
                            attributes.push(Attribute {
                                name: "rtpmap".to_owned(),
                                value: Some("0 PCMU/8000".to_owned()),
                            });
                        }
                        "PCMA" => {
                            attributes.push(Attribute {
                                name: "rtpmap".to_owned(),
                                value: Some("8 PCMA/8000".to_owned()),
                            });
                        }
                        "GSM" => {
                            attributes.push(Attribute {
                                name: "rtpmap".to_owned(),
                                value: Some("3 GSM/8000".to_owned()),
                            });
                        }
                        "opus" => {
                            attributes.push(Attribute {
                                name: "rtpmap".to_owned(),
                                value: Some("96 opus/48000/2".to_owned()),
                            });
                        }
                        "telephone-event" => {
                            attributes.push(Attribute {
                                name: "rtpmap".to_owned(),
                                value: Some("101 telephone-event/8000".to_owned()),
                            });
                            attributes.push(Attribute {
                                name: "fmtp".to_owned(),
                                value: Some("101 0-16".to_owned()),
                            });
                        }
                        _ => {
                            attributes.push(Attribute {
                                name: "rtpmap".to_owned(),
                                value: Some(format!(
                                    "{}/{}/{}/{}/",
                                    codec.pt(),
                                    codec.name(),
                                    codec.clock_rate(),
                                    codec.channels()
                                )),
                            });
                        }
                    }
                    media_formats.push(codec.pt().to_string());
                }

                MediaDescription {
                    media_type: sdp_media_stream.media_type,
                    proto: sdp_media_stream.transport,
                    port: sdp_media_stream.port,
                    number_of_ports: None,
                    media_formats,
                    title: None,
                    connection_information: None,
                    bandwidth_information: vec![],
                    attributes,
                }
            })
            .collect();

        if let Some(m) = media.last_mut() {
            m.attributes.push(Attribute {
                name: "ptime".to_owned(),
                value: Some("20".to_owned()),
            });

            m.attributes.push(Attribute {
                name: "maxptime".to_owned(),
                value: Some("150".to_owned()),
            });

            m.attributes.push(Attribute {
                name: config.direction.to_string(),
                value: None,
            });
        }

        SessionDescription {
            origin: Origin {
                user: "-".to_owned(),
                session_id: rand::random::<u64>(),
                session_version: rand::random::<u64>(),
                nettype: NetType::IN,
                addrtype: if config.origin_ip.is_ipv4() {
                    AddrType::IP4
                } else {
                    AddrType::IP6
                },
                unicast_address: config.origin_ip.to_string(),
            },
            session_name: "-".to_owned(),
            session_information: None,
            uri: None,
            email_address: None,
            phone_number: None,
            connection_information: Some(ConnectionInformation {
                nettype: NetType::IN,
                addrtype: if config.origin_ip.is_ipv4() {
                    AddrType::IP4
                } else {
                    AddrType::IP6
                },
                connection_address: config.origin_ip,
            }),
            bandwidth_information: vec![],
            attributes: vec![],
            time: vec![TimeDescription {
                time_active: TimeActive {
                    start_time: 0,
                    stop_time: 0,
                },
                repeat_times: vec![],
            }],
            media,
        }
    }

    pub fn local_sdp(&self) -> Option<&SessionDescription> {
        match self {
            Self::LocalOffered { offer, .. } => Some(offer),
            Self::Complete {
                offer,
                role: Role::Offerer,
                ..
            } => Some(offer),
            Self::Complete {
                answer,
                role: Role::Answerer,
                ..
            } => Some(answer),
            _ => None,
        }
    }

    pub fn remote_sdp(&self) -> Option<&SessionDescription> {
        match self {
            Self::RemoteOffered { offer, .. } => Some(offer),
            Self::Complete {
                offer,
                role: Role::Answerer,
                ..
            } => Some(offer),
            Self::Complete {
                answer,
                role: Role::Offerer,
                ..
            } => Some(answer),
            _ => None,
        }
    }

    pub fn answer(&self) -> Option<&SessionDescription> {
        match self {
            Self::Complete { answer, .. } => Some(answer),
            _ => None,
        }
    }
}

impl SdpConfig {
    pub fn new(origin_ip: IpAddr, direction: Direction) -> Self {
        Self {
            origin_ip,
            direction,
            media_streams: vec![],
        }
    }

    pub fn add_media_stream(mut self, media_stream: SdpMediaStream) -> Self {
        self.media_streams.push(media_stream);
        self
    }

    pub fn ip(&self) -> IpAddr {
        self.origin_ip
    }

    pub fn builder() -> SdpConfigBuilder {
        Default::default()
    }
}

impl Default for SdpConfigBuilder {
    fn default() -> Self {
        SdpConfigBuilder {
            address: IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            direction: Direction::SendRecv,
            port: 0,
            codecs: vec![],
            transport: SdpTransport::RTPAVP,
        }
    }
}

impl SdpConfigBuilder {
    pub fn address(mut self, addr: impl Into<IpAddr>) -> Self {
        self.address = addr.into();
        self
    }

    pub fn direction(mut self, d: Direction) -> Self {
        self.direction = d;
        self
    }

    pub fn audio(self, port: u16, codecs: impl IntoIterator<Item = Codec>) -> Self {
        self.audio_with_transport(SdpTransport::RTPAVP, port, codecs)
    }

    pub fn audio_with_transport(
        mut self,
        transport: SdpTransport,
        port: u16,
        codecs: impl IntoIterator<Item = Codec>,
    ) -> Self {
        self.port = port;
        self.codecs = codecs.into_iter().collect();
        self.transport = transport;
        self
    }

    pub fn build(self) -> SdpConfig {
        SdpConfig {
            origin_ip: self.address,
            direction: self.direction,
            media_streams: vec![
                SdpMediaStream::audio(self.port, self.transport).with_codecs(self.codecs),
            ],
        }
    }
}

impl SdpMediaStream {
    pub fn new(media_type: MediaType, transport: SdpTransport, port: u16) -> Self {
        Self {
            codecs: vec![],
            transport,
            port,
            media_type,
        }
    }

    pub fn audio(port: u16, transport: SdpTransport) -> Self {
        Self {
            transport,
            media_type: MediaType::Audio,
            codecs: vec![],
            port,
        }
    }
    pub fn video(port: u16, transport: SdpTransport) -> Self {
        Self {
            transport,
            media_type: MediaType::Video,
            codecs: vec![],
            port,
        }
    }

    pub fn with_codecs(mut self, codecs: Vec<Codec>) -> Self {
        self.codecs = codecs;
        self
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

#[cfg(test)]
mod tests {
    use utils::encode::Encode;

    use super::*;
    use crate::codec::Codec;
    use crate::sdp::Direction;

    #[test]
    fn test_generate_offer() {
        let mut negotiator = Negotiator::new();

        let offer_params = SdpConfig::new(IpAddr::from([127, 0, 0, 1]), Direction::SendRecv)
            .add_media_stream(
                SdpMediaStream::audio(34391, SdpTransport::RTPAVP)
                    .with_codecs(vec![Codec::ULAW, Codec::ALAW]),
            );

        let offer = negotiator.generate_offer(&offer_params).unwrap();

        println!("{}", offer.encode().unwrap());
    }
}
