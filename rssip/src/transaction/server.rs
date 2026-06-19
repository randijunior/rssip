use tokio::sync::mpsc::{self};
use tokio::task;
use tokio::time::{self, Instant};

use crate::endpoint::Endpoint;
use crate::error::{Error, Result};
use crate::message::ReasonPhrase;
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transaction::fsm::{State, StateMachine};
use crate::transaction::timers::{T1, T2, T4};
use crate::transaction::{TransactionEntry, TransactionKey};
use crate::transport::incoming::{IncomingMessage, IncomingRequest};
use crate::transport::outgoing::{OutgoingDestInfo, OutgoingResponse, TargetTransportInfo};

const CHANNEL_RECV_BUG: &str = "receiver should exist when no provisional response is sent";

/// A Server Transaction.
///
/// Represents a SIP server transaction.
pub struct ServerTransaction {
    transaction_key: TransactionKey,
    endpoint: Endpoint,
    state_machine: StateMachine,
    request: IncomingRequest,
    target_info: Option<TargetTransportInfo>,
    receiver: Option<mpsc::Receiver<IncomingMessage>>,
    provisonal_retrans_handle: Option<ProvisionalRetransHandle>,
}

impl ServerTransaction {
    /// Create a new [`ServerTransaction`] from the given request.
    ///
    /// # Panics
    ///
    /// Panics if request method is `ACK`.
    pub(crate) fn from_request(request: IncomingRequest, endpoint: Endpoint) -> Self {
        let method = request.req_line.method;
        assert_ne!(
            method,
            SipMethod::Ack,
            "ACK requests do not create transactions"
        );

        let initial_state = if method == SipMethod::Invite {
            State::Proceeding
        } else {
            State::Trying
        };
        let state_machine = StateMachine::new(initial_state);
        let (sender, receiver) = mpsc::channel(10);

        let transaction_key = TransactionKey::from_request(&request);

        endpoint
            .tsx_plugin()
            .add_transaction(transaction_key.clone(), sender);

        let server_tsx = Self {
            endpoint,
            transaction_key,
            request,
            state_machine,
            receiver: Some(receiver),
            target_info: None,
            provisonal_retrans_handle: None,
        };

        log::trace!(
            "Server transaction created [{:#?}] ({})",
            method,
            server_tsx
                .request
                .incoming_info
                .mandatory_headers
                .call_id
                .id()
        );

        server_tsx
    }

    /// Sends a provisional response with the given `status`.
    ///
    /// This is a shortcut for:
    ///
    /// ```no_run
    /// let response = transaction.create_response(status, None);
    /// transaction.send_provisional_response(response).await;
    /// ```
    /// See [`send_provisional_response`](Self::send_provisional_response) for more info.
    pub async fn send_provisional_status(&mut self, status: StatusCode) -> Result<()> {
        let response = self.create_response(status, None);

        self.send_provisional_response(response).await?;

        Ok(())
    }

    /// Sends a provisional response.
    ///
    /// # Panics
    ///
    /// Panics if the `response` is not provisional (`1xx`).
    pub async fn send_provisional_response(
        &mut self,
        mut response: OutgoingResponse,
    ) -> Result<()> {
        let code = response.status_line.code;

        if !code.is_provisional() {
            return Err(Error::Other(format!(
                "Invalid provisional response (expected 1xx) got {code:?}"
            )));
        }

        self.send_response(&mut response).await?;

        if let Some(ref mut handle) = self.provisonal_retrans_handle {
            handle
                .provisional_tx
                .send(response)
                .map_err(|_| Error::ChannelClosed)?
        } else {
            let handle = self.spawn_retransmit_provisional_task(response);
            self.provisonal_retrans_handle = Some(handle);
        }

        Ok(())
    }

    /// Sends a final response with the given `status`.
    ///
    /// This is a shortcut for:
    ///
    /// ```no_run
    /// let response = transaction.create_response(status, None);
    /// transaction.send_final_response(response).await;
    /// ```
    /// See [`send_final_response`](Self::send_final_response) for more info.
    pub async fn send_final_status(self, status: StatusCode) -> Result<()> {
        let response = self.create_response(status, None);

        self.send_final_response(response).await?;

        Ok(())
    }

    /// Sends a final response.
    ///
    /// # Panics
    ///
    /// Panics if the `response` is not final (`2xx-6xx`).
    pub async fn send_final_response(mut self, mut response: OutgoingResponse) -> Result<()> {
        let code = response.status_line.code;

        if !code.is_final() {
            return Err(Error::Other(format!(
                "Invalid final response (expected 2xx-6xx) got {code:?}"
            )));
        }

        let is_invite_tsx = self.is_invite_tsx();

        self.send_response(&mut response).await?;

        // INVITE - 2xx from TU send response --> Terminated
        if is_invite_tsx && matches!(code.as_u16(), 200..299) {
            self.set_state(State::Terminated);
            return Ok(());
        }
        // INVITE - 300-699 from TU send response --> Completed
        // non-INVITE - 200-699 from TU send response --> Completed
        self.set_state(State::Completed);

        // INVITE/non-INVITE - reliable --> timer_k/timer_j fire in zero seconds.
        if self.is_reliable() {
            self.set_state(State::Terminated);
            return Ok(());
        }
        // timer_k/timer_j
        let deadline = Instant::now() + if is_invite_tsx { T4 } else { T1 * 64 };

        let mut channel = if let Some(task) = self.provisonal_retrans_handle.take() {
            task.join_handle.await.map_err(std::io::Error::from)?
        } else {
            self.receiver.take().expect(CHANNEL_RECV_BUG)
        };

        if is_invite_tsx && !self.is_reliable() {
            tokio::spawn(async move {
                let mut retrans_interval = T1;
                loop {
                    let fut = time::timeout(retrans_interval, channel.recv());

                    match time::timeout_at(deadline, fut).await {
                        Ok(Ok(Some(IncomingMessage::Request(req))))
                            if req.req_line.method == SipMethod::Ack =>
                        {
                            self.set_state(State::Confirmed);
                            time::sleep(T4).await;
                            self.set_state(State::Terminated);
                            break;
                        }
                        Ok(Ok(Some(_msg))) => {
                            _ = self.send_response(&mut response).await;
                            continue;
                        }
                        Ok(Err(_elapsed)) => {
                            // retransmit
                            _ = self.send_response(&mut response).await;
                            retrans_interval = std::cmp::min(retrans_interval * 2, T2);
                            continue;
                        }
                        Err(_elapsed) => {
                            self.set_state(State::Terminated);
                            break;
                        }
                        Ok(Ok(None)) => break,
                    }
                }
            });
        } else {
            tokio::spawn(async move {
                while let Ok(Some(_)) = time::timeout_at(deadline, channel.recv()).await {
                    if let Err(err) = self.send_response(&mut response).await {
                        log::error!("Failed to send response: {}", err);
                    }
                }
                self.set_state(State::Terminated);
            });
        }

        Ok(())
    }

    pub fn create_response(
        &self,
        code: StatusCode,
        reason: Option<ReasonPhrase>,
    ) -> OutgoingResponse {
        if let Some(target) = self.target_info.clone() {
            let source_addr = target.transport.local_addr();
            let response = self.endpoint.create_response(&self.request, code, reason);

            let dest_info = OutgoingDestInfo {
                host_port: (source_addr.into(), target.transport.protocol()),
                transport: Some(target),
            };

            OutgoingResponse {
                response,
                dest_info,
                encoded: Default::default(),
            }
        } else {
            self.endpoint
                .create_outgoing_response(&self.request, code, reason)
        }
    }

    pub(crate) fn request(&self) -> &IncomingRequest {
        &self.request
    }

    pub fn is_invite_tsx(&self) -> bool {
        self.request.req_line.method == SipMethod::Invite
    }

    pub(crate) fn key(&self) -> &TransactionKey {
        &self.transaction_key
    }

    pub(crate) fn get_entry(&self) -> TransactionEntry {
        self.endpoint
            .tsx_plugin()
            .get_entry(self.key())
            .expect("must exists while server tsx is not dropped")
    }

    fn set_state(&mut self, state: State) {
        self.state_machine.set_state(state);
    }

    pub fn state_machine_mut(&mut self) -> &mut StateMachine {
        &mut self.state_machine
    }

    async fn send_response(&mut self, response: &mut OutgoingResponse) -> Result<()> {
        self.endpoint.send_outgoing_response(response).await?;

        if self.target_info.is_none() {
            self.target_info = response.dest_info.transport.clone();
        }
        Ok(())
    }

    fn is_reliable(&self) -> bool {
        self.request
            .incoming_info
            .transport_msg
            .transport
            .is_reliable()
    }

    fn spawn_retransmit_provisional_task(
        &mut self,
        mut response: OutgoingResponse,
    ) -> ProvisionalRetransHandle {
        let mut receiver = self.receiver.take().expect(CHANNEL_RECV_BUG);

        self.set_state(State::Proceeding);

        let mut state_rx = self.state_machine.subscribe_state();
        let (provisional_tx, mut tu_provisional_rx) = mpsc::unbounded_channel();

        let endpoint_clone = self.endpoint.clone();
        let join_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;

                    _= state_rx.recv() => {
                        log::debug!("Leaving Proceding State...");
                        return receiver;
                    }
                    Some(new_tu_provisional) = tu_provisional_rx.recv() => {
                        response = new_tu_provisional;
                    }
                    Some(_msg) = receiver.recv() => {
                           if let Err(err) = endpoint_clone.send_outgoing_response(&mut response)
                           .await {
                            log::error!("Failed to retransmit: {}", err);
                           }
                    }
                }
            }
        });

        ProvisionalRetransHandle {
            provisional_tx,
            join_handle,
        }
    }
}

struct ProvisionalRetransHandle {
    join_handle: task::JoinHandle<mpsc::Receiver<IncomingMessage>>,
    provisional_tx: mpsc::UnboundedSender<OutgoingResponse>,
}

impl Drop for ServerTransaction {
    fn drop(&mut self) {
        self.endpoint
            .tsx_plugin()
            .remove_transaction(&self.transaction_key);
        log::trace!(
            "Server transaction destroyed [{:#?}] ({})",
            self.request.req_line.method,
            self.request.incoming_info.mandatory_headers.call_id.id()
        );
    }
}
