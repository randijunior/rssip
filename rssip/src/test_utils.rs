//! Test utilities for all unit tests in the library.

use std::str::FromStr;

use bytes::Bytes;

use crate::dialog::DialogPlugin;
use crate::endpoint::{Endpoint, EndpointBuilder};
use crate::message::Request;
use crate::message::headers::{CSeq, CallId, From, Header, Headers, MaxForwards, To, Via};
use crate::message::method::SipMethod;
use crate::message::sip_uri::Uri;
use crate::transaction::TsxPlugin;
use crate::transport::incoming::{IncomingInfo, IncomingRequest, MandatoryHeaders};
use crate::transport::{Packet, Transport, TransportMessage};

pub async fn create_test_endpoint() -> Endpoint {
    EndpointBuilder::new()
        .with_udp_addr("127.0.0.1:0")
        .with_plugin(DialogPlugin::default())
        .with_plugin(TsxPlugin::default())
        .build()
        .await
        .unwrap()
}

fn create_test_headers(method: SipMethod) -> Headers {
    let branch = crate::generate_branch();

    let via = Via::from_str(&format!(
        "SIP/2.0/UDP localhost:5060;branch={branch};received=127.0.0.1"
    ))
    .unwrap();
    let from = From::from_str("Alice <sip:alice@localhost>;tag=1928301774").unwrap();
    let to = To::from_str("Bob <sip:bob@localhost>").unwrap();
    let cid = CallId::from("a84b4c76e66710@pc33.atlanta.com");
    let mfowards = MaxForwards::new(70);
    let cseq = CSeq::new(1, method);

    crate::headers! {
        Header::Via(via),
        Header::From(from),
        Header::To(to),
        Header::CallId(cid),
        Header::CSeq(cseq),
        Header::MaxForwards(mfowards)
    }
}

pub fn create_test_request(method: SipMethod, transport: Transport) -> IncomingRequest {
    let headers = create_test_headers(method);
    let target = format!("sip:{}", transport.local_addr());
    let uri = Uri::from_str(&target).unwrap();

    let mandatory_headers = MandatoryHeaders::from_headers(&headers).unwrap();

    let request = Request::with_headers(method, uri, headers);
    let packet = Packet::new(Bytes::new(), transport.local_addr());

    let transport_msg = TransportMessage { packet, transport };

    let incoming_info = IncomingInfo {
        transport_msg,
        mandatory_headers,
    };

    IncomingRequest {
        request,
        incoming_info: Box::new(incoming_info),
    }
}

pub mod parser {
    /// Expands to a test function that validates if a given `input` string
    /// is a valid SIP URI and matches the `expected` structure.
    ///
    /// # Example
    ///
    /// ```rust
    /// uri_test_ok!(
    ///     name: test_simple_uri,
    ///     input: "sip:alice@atlanta.com",
    ///     expected: my_expected_uri_struct
    /// );
    /// ```
    #[macro_export]
    macro_rules! uri_test_ok {
        (name: $name:ident, input: $input:literal, expected: $expected:expr) => {
            #[test]
            fn $name() -> Result<()> {
                let uri = $crate::parser::SipParser::new($input).parse_sip_uri(true)?;

                assert_eq!($expected.scheme, uri.scheme());
                assert_eq!($expected.host_port.host, uri.host_port().host);
                assert_eq!($expected.host_port.port, uri.host_port().port);
                assert_eq!($expected.user, uri.user().cloned());
                assert_eq!($expected.transport_param, uri.transport_param());
                assert_eq!($expected.ttl_param, uri.ttl_param());
                assert_eq!($expected.method_param, uri.method_param());
                assert_eq!($expected.user_param.as_deref(), uri.user_param());
                assert_eq!($expected.lr_param, uri.lr_param());
                assert_eq!(&$expected.maddr_param, uri.maddr_param());

                for param in $expected.params.iter() {
                    assert_eq!($expected.params.param(&param.name), param.value.as_deref());
                }
                if let Some(headers) = uri.headers() {
                    assert!($expected.headers.is_some(), "missing headers!");
                    for param in $expected.headers.unwrap().iter() {
                        assert_eq!(headers.param(&param.name), param.value.as_deref());
                    }
                }

                Ok(())
            }
        };
    }
}

pub mod transaction {
    use std::cmp;
    use std::net::SocketAddr;
    use std::time::Duration;

    use tokio::sync::mpsc;
    use tokio::task;
    use tokio::time::{self};

    use super::transport::MockTransport;
    use super::{create_test_endpoint, create_test_request};
    use crate::endpoint::Endpoint;
    use crate::message::Request;
    use crate::message::method::SipMethod;
    use crate::message::status_code::StatusCode;
    use crate::transaction::ServerTransaction;
    use crate::transaction::client::ClientTransaction;
    use crate::transaction::fsm::{self};
    use crate::transaction::timers::{T1, T2, T4};
    use crate::transport::incoming::{
        IncomingInfo, IncomingMessage, IncomingRequest, IncomingResponse,
    };
    use crate::transport::{Packet, Transport, TransportMessage};

    pub const CODE_100_TRYING: StatusCode = StatusCode::Trying;
    pub const CODE_180_RINGING: StatusCode = StatusCode::Ringing;
    pub const CODE_202_ACCEPTED: StatusCode = StatusCode::Accepted;
    pub const CODE_301_MOVED_PERMANENTLY: StatusCode = StatusCode::MovedPermanently;
    pub const CODE_404_NOT_FOUND: StatusCode = StatusCode::NotFound;
    pub const CODE_504_SERVER_TIMEOUT: StatusCode = StatusCode::ServerTimeout;
    pub const CODE_603_DECLINE: StatusCode = StatusCode::Decline;

    /// Asserts that the last state received in the [`broadcast::Receiver<State>`] are equal to the expected.
    #[macro_export]
    macro_rules! assert_eq_state {
        ($watcher:expr, $state:expr $(,)?) => {
            $crate::assert_eq_state!($watcher, $state,)
        };
        ($watcher:expr, $state:expr, $($arg:tt)+) => {{
            let new_state = $crate::test_utils::transaction::wait_state_change(&mut $watcher).await;
            assert_eq!(new_state, $state, $($arg)+);
        }};
    }

    pub async fn wait_state_change(state: &mut fsm::TsxStateChangeReceiver) -> fsm::State {
        match time::timeout(Duration::from_millis(50), state.recv()).await {
            Ok(Err(err)) => panic!("{}", format!("The channel has been closed: {err}")),
            Err(_) => panic!("timeout!"),
            Ok(Ok(state)) => state,
        }
    }

    pub struct FakeUAS {
        pub sender: mpsc::Sender<IncomingMessage>,
        pub request: IncomingRequest,
        pub endpoint: Endpoint,
    }

    impl FakeUAS {
        pub async fn respond(&self, code: StatusCode) {
            let mandatory_headers = self.request.incoming_info.mandatory_headers.clone();
            let outgoing = self
                .endpoint
                .create_outgoing_response(&self.request, code, None);

            let packet = Packet::new(
                outgoing.encoded,
                self.request.incoming_info.transport_msg.packet.source,
            );

            let transport_msg = TransportMessage {
                packet,
                transport: self.request.incoming_info.transport_msg.transport.clone(),
            };
            let info = IncomingInfo {
                transport_msg,
                mandatory_headers,
            };

            let response = IncomingResponse {
                response: outgoing.response,
                incoming_info: Box::new(info),
            };

            let transaction_message = IncomingMessage::Response(response);

            self.sender.send(transaction_message).await.unwrap();
        }
    }

    pub struct FakeUAC {
        pub sender: mpsc::Sender<IncomingMessage>,
        pub request: IncomingRequest,
    }

    impl FakeUAC {
        pub async fn retransmit_n_times(&self, n: usize) {
            for _ in 0..n {
                self.retransmit().await;
            }
        }

        pub async fn retransmit(&self) {
            self.send(self.request.clone()).await;
        }

        pub async fn send_ack_request(&mut self) {
            let mut incoming = self.request.clone();
            incoming.request.req_line.method = SipMethod::Ack;
            self.send(incoming).await;
        }

        async fn send(&self, request: IncomingRequest) {
            self.sender
                .send(IncomingMessage::Request(request))
                .await
                .unwrap();
            tokio::task::yield_now().await;
        }
    }

    pub struct TestTimer {
        retrans_interval: Duration,
    }

    impl TestTimer {
        pub fn new() -> Self {
            Self {
                retrans_interval: T1,
            }
        }

        pub async fn timer_h(&self) {
            tokio::time::sleep(T1 * 64).await
        }

        pub async fn timer_j(&self) {
            tokio::time::sleep(T1 * 64).await
        }

        pub async fn timer_k(&self) {
            tokio::time::sleep(T1 * 64).await
        }

        pub async fn timer_d(&self) {
            tokio::time::sleep(T1 * 64).await
        }

        pub async fn timer_i(&self) {
            tokio::time::sleep(T4).await
        }

        fn next_interval(&mut self) {
            self.retrans_interval = cmp::min(self.retrans_interval * 2, T2);
        }

        async fn wait_interval(&self) {
            time::sleep(self.retrans_interval).await;
        }

        pub async fn wait_for_retransmissions(&mut self, n: usize) {
            for _ in 0..n {
                self.wait_interval().await;
                self.next_interval();
                task::yield_now().await;
            }
        }
    }

    pub struct SendRequestContext {
        pub endpoint: Endpoint,
        pub request: Request,
        pub transport: Transport,
        pub destination: SocketAddr,
    }

    impl SendRequestContext {
        pub async fn setup(method: SipMethod) -> Self {
            let transport = Transport::new(MockTransport::new_udp());

            let endpoint = create_test_endpoint().await;
            let incoming = create_test_request(method, transport.clone());

            let destination = incoming.incoming_info.transport_msg.packet.source;
            let request = incoming.request;

            Self {
                endpoint,
                request,
                transport,
                destination,
            }
        }
    }

    pub struct ClientTestContext {
        pub client: ClientTransaction,
        pub server: FakeUAS,
        pub transport: MockTransport,
        pub timer: TestTimer,
        pub state: fsm::TsxStateChangeReceiver,
    }

    impl ClientTestContext {
        pub async fn setup(method: SipMethod) -> Self {
            Self::new(method, MockTransport::new_udp()).await
        }

        pub async fn setup_reliable(method: SipMethod) -> Self {
            Self::new(method, MockTransport::new_tcp()).await
        }

        async fn new(method: SipMethod, transport: MockTransport) -> Self {
            let transport_impl = Transport::new(transport.clone());
            let timer = TestTimer::new();

            let endpoint = create_test_endpoint().await;
            let request = create_test_request(method, transport_impl.clone());

            let destination = request.incoming_info.transport_msg.packet.source;

            let target = (transport_impl, destination);

            let mut client = ClientTransaction::send_request_with_target(
                request.request.clone(),
                endpoint.clone(),
                target,
            )
            .await
            .expect("failure sending request");

            let expected_state = if method == SipMethod::Invite {
                fsm::State::Calling
            } else {
                fsm::State::Trying
            };

            assert_eq!(
                client.state(),
                expected_state,
                "Transaction state should transition to {expected_state} after sending request"
            );

            let sender = endpoint.tsx_plugin().get_entry(client.key()).unwrap();

            let server = FakeUAS {
                sender,
                request,
                endpoint,
            };

            let state = client.state_machine_mut().subscribe_state();

            Self {
                client,
                server,
                transport,
                timer,
                state,
            }
        }
    }

    pub struct ServerTestContext {
        pub server: ServerTransaction,
        pub client: FakeUAC,
        pub transport: MockTransport,
        pub timer: TestTimer,
        pub state: fsm::TsxStateChangeReceiver,
    }

    impl ServerTestContext {
        pub async fn setup(method: SipMethod) -> Self {
            Self::new(method, MockTransport::new_udp()).await
        }

        pub async fn setup_reliable(method: SipMethod) -> Self {
            Self::new(method, MockTransport::new_tcp()).await
        }

        async fn new(method: SipMethod, transport: MockTransport) -> Self {
            let transport_impl = Transport::new(transport.clone());

            let endpoint = create_test_endpoint().await;
            let request = create_test_request(method, transport_impl);

            let mut server = ServerTransaction::from_request(request.clone(), endpoint.clone());

            let sender = endpoint.tsx_plugin().get_entry(server.key()).unwrap();

            let client = FakeUAC { sender, request };

            let timer = TestTimer::new();

            let state = server.state_machine_mut().subscribe_state();

            Self {
                server,
                client,
                transport,
                timer,
                state,
            }
        }
    }
}

pub mod transport {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::{Arc, Mutex};

    use crate::message::{Request, SipMessage};
    use crate::parser::SipParser;
    use crate::transport::{SipTransport, TransportProtocol};

    /// A mock transport, for testing purposes
    #[derive(Clone)]
    pub struct MockTransport {
        sent: Arc<Mutex<Vec<(Vec<u8>, SocketAddr)>>>,
        addr: SocketAddr,
        protocol: TransportProtocol,
        fail_at: Option<usize>,
    }

    impl MockTransport {
        pub fn with_transport_type(protocol: TransportProtocol) -> Self {
            let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
            let port = protocol.default_port();
            let mock = Self {
                sent: Default::default(),
                addr: SocketAddr::new(ip, port),
                protocol,
                fail_at: None,
            };

            mock
        }

        pub fn new_udp() -> Self {
            Self::with_transport_type(TransportProtocol::Udp)
        }

        pub fn new_tcp() -> Self {
            Self::with_transport_type(TransportProtocol::Tcp)
        }

        pub fn new_tls() -> Self {
            Self::with_transport_type(TransportProtocol::Tls)
        }

        pub fn sent_count(&self) -> usize {
            self.sent.lock().unwrap().len()
        }

        pub fn get_last_sent_request(&self) -> Option<Request> {
            self.get_last_sent_message().map(|msg| {
                if let SipMessage::Request(req) = msg {
                    Some(req)
                } else {
                    None
                }
            })?
        }

        pub fn last_buffer(&self) -> Option<Vec<u8>> {
            let guard = self.sent.lock().unwrap();
            guard.last().map(|(buff, _)| buff).cloned()
        }

        pub fn get_last_sent_message(&self) -> Option<SipMessage> {
            self.last_buffer().map(|b| SipParser::parse(&b).unwrap())
        }

        fn push_msg(&self, (buf_vec, address): (Vec<u8>, SocketAddr)) -> usize {
            let mut guard = self.sent.lock().unwrap();
            guard.push((buf_vec, address));
            guard.len()
        }
    }

    #[async_trait::async_trait]
    impl SipTransport for MockTransport {
        async fn send_msg(&self, buf: &[u8], address: &SocketAddr) -> crate::Result<usize> {
            let current_count = self.push_msg((buf.to_vec(), *address));

            if let Some(fail_at) = self.fail_at
                && fail_at == current_count
            {
                return Err(crate::Error::TransportError("Simulated failure".into()));
            }

            Ok(buf.len())
        }

        fn remote_addr(&self) -> Option<SocketAddr> {
            None
        }

        fn protocol(&self) -> TransportProtocol {
            self.protocol
        }

        fn local_addr(&self) -> SocketAddr {
            self.addr
        }

        fn is_reliable(&self) -> bool {
            self.protocol.is_reliable()
        }

        fn is_secure(&self) -> bool {
            self.protocol.is_secure()
        }
    }
}
