use std::error::Error;

use tracing::Level;
use tracing_subscriber::fmt::time::ChronoLocal;
use voip::sip::endpoint::{self, Endpoint, ReceivedRequest};
use voip::sip::message::method::SipMethod;
use voip::sip::message::status_code::StatusCode;

pub struct SipStateless;

#[async_trait::async_trait]
impl endpoint::Plugin for SipStateless {
    fn name(&self) -> &'static str {
        "sip-stateless"
    }

    async fn on_receive_request(&self, mut received: ReceivedRequest<'_>, endpoint: &Endpoint) {
        let request = received.take();

        if request.req_line.method != SipMethod::Ack {
            let mut response =
                endpoint.create_outgoing_response(&request, StatusCode::NotImplemented, None);

            endpoint
                .send_outgoing_response(&mut response)
                .await
                .unwrap();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_env_filter("voip=trace")
        .with_timer(ChronoLocal::new(String::from("%H:%M:%S%.3f")))
        .init();

    let endpoint = Endpoint::builder()
        .with_plugin(SipStateless)
        .with_udp_addr("0.0.0.0:8089")
        .build()
        .await?;

    endpoint.run_forever().await?;

    Ok(())
}
