use std::error::Error;
use std::sync::atomic::{AtomicU16, Ordering};

use rssip::IncomingRequest;
use rssip::endpoint::{self, Endpoint, Takeable};
use rssip::media::codec::Codec;
use rssip::media::negotiator::{SdpMediaStream, SdpOfferParams};
use rssip::media::sdp::{Direction, SdpTransport};
use rssip::message::SipBody;
use rssip::message::headers::Contact;
use rssip::message::method::SipMethod;
use rssip::message::status_code::StatusCode;
use rssip::transaction::{ServerTransaction, TsxPlugin};
use rssip::ua::dialog::DialogPlugin;
use rssip::ua::session::{
    Session, SessionEventHandler, SessionNegotiationDoneEvent, SessionTerminatedEvent,
};
use rssip::utils::local_ip::get_local_ip_addr;
use tracing::Level;
use tracing_subscriber::fmt::time::ChronoLocal;

pub struct Acceptor {
    contact: Contact,
    media_port: AtomicU16,
}

#[async_trait::async_trait]
impl endpoint::Plugin for Acceptor {
    fn name(&self) -> &'static str {
        "acceptor"
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
        let contact_clone = self.contact.clone();

        let mut session = Session::from_invite(request, contact_clone, endpoint.clone()).unwrap();

        session.progress(StatusCode::Trying, None).await.unwrap();

        let sdp = SdpOfferParams::new(get_local_ip_addr(), Direction::SendRecv);

        let media_port = self.media_port.fetch_add(1, Ordering::SeqCst);

        let sdp = sdp.add_media_stream(
            SdpMediaStream::audio(media_port, SdpTransport::RTPAVPF)
                .with_codecs(vec![Codec::ULAW, Codec::ALAW]),
        );

        session
            .accept(StatusCode::Ok, None, &sdp, MyHandler)
            .await
            .unwrap();
    }
}

pub struct MyHandler;

impl SessionEventHandler for MyHandler {
    async fn on_terminated(&self, evt: SessionTerminatedEvent) {
        println!("Terminated, cause = {:#?}", evt.cause);
    }
    async fn on_negotiation_done(&self, _evt: SessionNegotiationDoneEvent<'_>) {
        println!("Sdp Negotiation Done");
        // Create RtpSession
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
            media_port: AtomicU16::new(1024),
            contact: "<sip:0.0.0.0:8089>".parse().unwrap(),
        })
        .with_udp_addr("0.0.0.0:8089")
        .build()
        .await?;

    tokio::signal::ctrl_c().await?;
    println!("shutting down");

    Ok(())
}
