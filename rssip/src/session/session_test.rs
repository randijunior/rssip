use super::*;
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::test_utils::transport::MockTransport;
use crate::test_utils::{create_test_endpoint, create_test_request};
use crate::transport::TransportHandle;

fn create_test_invite() -> IncomingRequest {
    let transport = TransportHandle::new(MockTransport::new_udp());
    create_test_request(SipMethod::Invite, transport)
}

#[tokio::test(start_paused = true)]
async fn test_create_session() {
    let endpoint = create_test_endpoint().await;
    let req = create_test_invite();
    let contact = "test <sip:localhost:5969>".parse().unwrap();
    let server_tsx = ServerTransaction::from_request(req, endpoint.clone());

    let mut session = Session::init_incoming(server_tsx, contact, endpoint).unwrap();

    session.progress(StatusCode::Trying).await.unwrap();
    session.progress(StatusCode::Ringing).await.unwrap();

    assert!(session.accept(StatusCode::Ok).await.is_err());
}
