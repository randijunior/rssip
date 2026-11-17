use std::error::Error;
use std::sync::atomic::{AtomicU16, Ordering};

use rssip::IncomingRequest;
use rssip::endpoint::{self, Endpoint, Takeable};
use rssip::media::codec::Codec;
use rssip::media::rtp::{RtpMessage, RtpSession};
use rssip::media::sdp::Direction;
use rssip::media::sdp::negotiator::SdpConfig;
use rssip::message::SipBody;
use rssip::message::headers::Contact;
use rssip::message::method::SipMethod;
use rssip::message::status_code::StatusCode;
use rssip::transaction::{ServerTransaction, TsxPlugin};
use rssip::ua::dialog::DialogPlugin;
use rssip::ua::session::{Session, SessionEvent};
use tracing::Level;
use tracing_subscriber::fmt::time::ChronoLocal;

pub struct Acceptor {
    contact: Contact,
    media_port: AtomicU16,
}

#[async_trait::async_trait]
impl endpoint::Plugin for Acceptor {
    fn name(&self) -> &'static str {
        "accept"
    }

    async fn incoming_request(&self, mut req: Takeable<'_, IncomingRequest>, endpoint: &Endpoint) {
        if req.req_line.method == SipMethod::Register {
            let req = req.take();

            let server_tsx = ServerTransaction::from_request(req, endpoint.clone());

            server_tsx.send_final_status(StatusCode::Ok).await.unwrap();

            return;
        }
        let request = if req.req_line.method == SipMethod::Invite {
            req.take()
        } else {
            return;
        };
        let contact = self.contact.clone();
        let port = self.media_port.fetch_add(1, Ordering::Relaxed);

        let mut session = Session::from_invite(request, contact, endpoint.clone()).unwrap();

        session.trying().await.unwrap();

        let sdp = SdpConfig::builder()
            .address([127, 0, 0, 1])
            .direction(Direction::SendRecv)
            .audio(port, [Codec::ULAW, Codec::ALAW])
            .build();

        let mut accepted = session.accept(&sdp).await.unwrap();

        let local = accepted.local_sdp();
        let remote = accepted.remote_sdp();

        let l = &local.media[0];
        let r = &remote.media[0];

        let l_ip = local.connection_addr(l).unwrap();
        let r_ip = remote.connection_addr(r).unwrap();

        let payload_type = l.media_formats[0].parse().unwrap();

        let mut rtp = RtpSession::create((l_ip, l.port), (r_ip, r.port), payload_type)
            .await
            .unwrap();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    evt = accepted.next_event() => match evt {
                        Some(SessionEvent::Terminated(cause)) => { println!("{cause:?}"); break; }
                        Some(_) => {}
                        None => break,
                    },
                    Some(pkt) = rtp.receive() => match pkt {
                        RtpMessage::Rtp(pkt) => {
                            rtp.write_rtp(&pkt.payload, 160).await.unwrap();
                        },
                        RtpMessage::Rtcp(_pkt) => ()
                    }
                }
            }
        });
    }
}

struct Logger;

#[async_trait::async_trait]
impl endpoint::Plugin for Logger {
    fn name(&self) -> &'static str {
        "logger"
    }

    async fn outgoing_response(&self, res: &mut rssip::OutgoingResponse) {
        let body_utf8 = get_body_utf8(&res.body);
        println!("{}{}{}", res.status_line, res.headers, body_utf8);
    }
    async fn incoming_request(&self, req: Takeable<'_, IncomingRequest>, _endpoint: &Endpoint) {
        let body_utf8 = get_body_utf8(&req.body);
        println!("{}{}{}", req.req_line, req.headers, body_utf8);
    }
}

fn get_body_utf8(body: &Option<SipBody>) -> &str {
    body.as_ref()
        .map(|b| std::str::from_utf8(&b).unwrap())
        .unwrap_or("")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_env_filter("rssip=trace")
        .with_timer(ChronoLocal::new(String::from("%H:%M:%S%.3f")))
        .init();

    let _endpoint = Endpoint::builder()
        .with_plugin(Logger)
        .with_plugin(DialogPlugin::default())
        .with_plugin(TsxPlugin::default())
        .with_plugin(Acceptor {
            media_port: AtomicU16::new(2048),
            contact: "<sip:0.0.0.0:8089>".parse().unwrap(),
        })
        .with_udp_addr("0.0.0.0:8089")
        .build()
        .await?;

    tokio::signal::ctrl_c().await?;
    println!("shutting down");

    Ok(())
}
