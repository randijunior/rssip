use media::sdp::SessionDescription;
use media::sdp::negotiator::{Negotiator, SdpConfig};
use media::sdp::parser::SdpParser;
use utils::encode::Encode;

use crate::message::headers::{Contact, ContentType, Header};
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::message::uri::SipUri;
use crate::message::{ReasonPhrase, SipBody};
use crate::transaction::{ClientTransaction, ServerTransaction};
use crate::ua::dialog::Dialog;
use crate::{Endpoint, Error, IncomingRequest, IncomingResponse, OutgoingRequest, Result};

// Offer                Answer             RFC    Ini Est Early
// -------------------------------------------------------------------
// 1. INVITE Req.          2xx INVITE Resp.     RFC 3261  Y   Y    N
// 2. 2xx INVITE Resp.     ACK Req.             RFC 3261  Y   Y    N

/// Represents a SIP Session.
pub struct Session<S> {
    state: S,
    negotiator: Negotiator,
}

pub struct Incoming {
    dialog: Dialog,
    server_tsx: ServerTransaction,
}

pub struct Calling {
    dialog: Dialog,
    client_tsx: ClientTransaction,
}

pub struct Accepted {
    dialog: Dialog,
}

pub struct InvitationParams {
    pub from_uri: SipUri,
    pub to_uri: SipUri,
    pub contact: Option<SipUri>,
    pub sdp: Option<SdpConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminatedCause {
    ByeReceived,
}

pub enum SessionEvent {
    Reinvite(IncomingRequest),
    Terminated(TerminatedCause),
    // Dtmf { digit: char, duration_ms: u32 },
}

impl Session<Calling> {
    // RFC 3261 13.2.1
    pub async fn send_invite(params: InvitationParams, endpoint: Endpoint) -> Result<Self> {
        let InvitationParams {
            from_uri,
            to_uri,
            contact,
            sdp,
        } = params;

        let mut dialog = Dialog::create_uac(from_uri, to_uri, contact, endpoint.clone());

        let mut request = dialog.create_request(SipMethod::Invite);

        let allow = endpoint.allow();
        let supported = endpoint.supported();

        if !allow.is_empty() {
            request.headers.push(Header::Allow(allow.clone()));
        }

        if !supported.is_empty() {
            request.headers.push(Header::Supported(supported.clone()));
        }

        let mut negotiator = Negotiator::default();

        if let Some(params) = sdp {
            let offer = negotiator.generate_offer(&params)?;
            let encoded = offer.encode()?;
            let sip_body = SipBody::from(bytes::Bytes::from(encoded));

            request
                .headers
                .push(Header::ContentType(ContentType::new_sdp()));

            request.body = Some(sip_body);
        }

        let client_tsx = ClientTransaction::send_request(request, endpoint).await?;

        Ok(Self {
            state: Calling { client_tsx, dialog },
            negotiator,
        })
    }

    pub async fn receive_provisional(&mut self) -> Result<Option<IncomingResponse>> {
        let Calling { client_tsx, .. } = &mut self.state;
        let response = client_tsx.receive_provisional_response().await?;
        Ok(response)
    }

    pub async fn receive_answer(mut self, sdp: Option<&SdpConfig>) -> Result<Session<Accepted>> {
        let Calling {
            mut dialog,
            client_tsx,
        } = self.state;

        let response = client_tsx.receive_final_response().await?;

        match response.status_line.code.as_u16() {
            // 13.2.2.4 2xx Responses
            200..=299 => {
                let ack_body = if let Some(body) = &response.body {
                    let remote_sdp = Self::parse_sdp(body)?;

                    match self.negotiator {
                        Negotiator::Idle => {
                            let Some(sdp) = sdp else {
                                return Err(Error::Custom(
                                    "offer required to answer a delayed offer".into(),
                                ));
                            };
                            let answer = self.negotiator.generate_answer(sdp)?;
                            let answer = answer.encode()?;

                            let ack_body = SipBody::from(bytes::Bytes::from(answer));

                            Some(ack_body)
                        }
                        Negotiator::LocalOffered { .. } => {
                            // This is an answer.
                            self.negotiator.receive_answer(remote_sdp)?;
                            None
                        }
                        Negotiator::RemoteOffered { .. } => {
                            todo!("we received early offer on 180 Ringing response (Early Media)")
                        }
                        _ => unreachable!(),
                    }
                } else {
                    None
                };

                let mut ack = dialog.create_request(SipMethod::Ack);
                ack.body = ack_body;

                let endpoint = dialog.endpoint();
                let mut outgoing = endpoint.create_outgoing_request(ack, None).await?;

                endpoint.send_outgoing_request(&mut outgoing).await?;

                Ok(Session {
                    state: Accepted { dialog },
                    negotiator: self.negotiator,
                })
            }
            // 13.2.2.2 3xx Responses
            300..=399 => todo!(),
            // 13.2.2.3 4xx, 5xx and 6xx Responses
            400..=699 => todo!(),
            _ => unreachable!("The response should always have a valid final status_code"),
        }
    }

    pub fn request(&self) -> &OutgoingRequest {
        self.state.client_tsx.request()
    }
}

impl Session<Incoming> {
    pub fn from_invite(
        request: IncomingRequest,
        contact: Contact,
        endpoint: Endpoint,
    ) -> Result<Self> {
        if request.req_line.method != SipMethod::Invite {
            return Err(Error::Custom(format!(
                "unexpected method '{}' expected INVITE",
                request.req_line.method
            )));
        }
        let dialog = Dialog::create_uas(&request, contact, endpoint.clone())?;

        let negotiator = if let Some(body) = &request.body {
            // EarlyOffer
            let remote_sdp = Self::parse_sdp(body)?;

            Negotiator::RemoteOffered { offer: remote_sdp }
        } else {
            Negotiator::Idle
        };

        let server_tsx = ServerTransaction::from_request(request, endpoint);

        Ok(Self {
            state: Incoming { server_tsx, dialog },
            negotiator,
        })
    }

    // RFC 3261 13.3.1.1
    pub async fn progress(
        &mut self,
        status_code: StatusCode,
        reason_phrase: Option<ReasonPhrase>,
    ) -> Result<()> {
        let Incoming { server_tsx, dialog } = &mut self.state;

        dialog
            .provisional_response(server_tsx, status_code, reason_phrase)
            .await?;

        Ok(())
    }

    pub async fn trying(&mut self) -> Result<()> {
        self.progress(StatusCode::Trying, Some("Trying".into()))
            .await?;
        Ok(())
    }

    pub async fn ringing(&mut self) -> Result<()> {
        self.progress(StatusCode::Ringing, Some("Ringing".into()))
            .await?;
        Ok(())
    }

    pub async fn accept(mut self, sdp: &SdpConfig) -> Result<Session<Accepted>> {
        let Incoming {
            server_tsx,
            mut dialog,
        } = self.state;

        let mut sip_response = dialog.create_response(&server_tsx, StatusCode::Ok, None);

        let our_sdp = if let Negotiator::RemoteOffered { .. } = self.negotiator {
            let answer = self.negotiator.generate_answer(sdp)?;
            answer.encode()?
        } else {
            let offer = self.negotiator.generate_offer(sdp)?;
            offer.encode()?
        };

        sip_response.body = Some(SipBody::from(bytes::Bytes::from(our_sdp)));

        sip_response
            .headers
            .push(Header::ContentType(ContentType::new_sdp()));

        server_tsx.send_final_response(sip_response).await?;

        let ack = dialog.wait_for_ack().await?;

        if let Negotiator::LocalOffered { .. } = self.negotiator {
            let Some(body) = &ack.body else {
                return Err(Error::Custom("missing answer on ack".into()));
            };
            let answer = Self::parse_sdp(body)?;
            self.negotiator.receive_answer(answer)?;
        }

        Ok(Session {
            state: Accepted { dialog },
            negotiator: self.negotiator,
        })
    }
}

impl Session<Accepted> {
    pub fn local_sdp(&self) -> &SessionDescription {
        self.negotiator.local_sdp().expect("a offer")
    }

    pub fn remote_sdp(&self) -> &SessionDescription {
        self.negotiator.remote_sdp().expect("a offer")
    }

    pub async fn next_event(&mut self) -> Option<SessionEvent> {
        let Accepted { dialog } = &mut self.state;

        let request = dialog.receive_request().await.ok()?;

        match request.req_line.method {
            SipMethod::Invite => {
                return Some(SessionEvent::Reinvite(request));
            }
            SipMethod::Bye => {
                let bye_tsx = ServerTransaction::from_request(request, dialog.endpoint().clone());

                dialog.final_response(bye_tsx, StatusCode::Ok).await.ok()?;

                return Some(SessionEvent::Terminated(TerminatedCause::ByeReceived));
            }
            method => {
                log::debug!("received request (ignoring): {}", method);
                return None;
            }
        }
    }
}

impl<S> Session<S> {
    fn parse_sdp(body: &SipBody) -> Result<SessionDescription> {
        let sdp = SdpParser::parse(body.as_ref())?;
        Ok(sdp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::method::SipMethod;
    use crate::test_utils::{create_test_endpoint, create_test_request};
    use crate::transport::{MockTransport, TransportHandle};

    fn create_test_invite() -> IncomingRequest {
        let transport = TransportHandle::new(MockTransport::new_udp());
        create_test_request(SipMethod::Invite, transport)
    }

    fn create_test_inv_params() -> InvitationParams {
        InvitationParams {
            from_uri: "Alice <sip:alice@example.com>".parse().unwrap(),
            to_uri: "Bob <sip:bob@example.com>".parse().unwrap(),
            contact: None,
            sdp: None,
        }
    }

    #[tokio::test]
    async fn test_server_session_late_offer() {
        let endpoint = create_test_endpoint().await;
        let request = create_test_invite();
        let contact = "test <sip:localhost:8089>".parse().unwrap();

        let _session = Session::from_invite(request, contact, endpoint).unwrap();
    }

    #[tokio::test]
    async fn test_client_session_late_offer() {
        let endpoint = create_test_endpoint().await;
        let params = create_test_inv_params();

        let _session = Session::send_invite(params, endpoint).await.unwrap();
    }
}
