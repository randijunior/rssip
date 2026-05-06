use tokio::sync::mpsc::{self};
use tokio::task;
use tokio::time::{self, Instant};

use crate::endpoint::Endpoint;
use crate::error::{Error, Result};
use crate::message::ReasonPhrase;
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transaction::fsm::{State, StateMachine};
use crate::transaction::manager::TransactionKey;
use crate::transaction::{T1, T2, T4, TransactionMessage};
use crate::transport::incoming::IncomingRequest;
use crate::transport::outgoing::{OutgoingDestInfo, OutgoingResponse, TargetTransportInfo};

/// A Server Transaction.
///
/// Represents a SIP server transaction.
pub struct ServerTransaction {
    transaction_key: TransactionKey,
    endpoint: Endpoint,
    state_machine: StateMachine,
    request: IncomingRequest,
    target_info: Option<TargetTransportInfo>,
    receiver: Option<mpsc::Receiver<TransactionMessage>>,
    provisonal_retrans_handle: Option<ProvisionalRetransHandle>,
}

impl ServerTransaction {
    /// Create a new [`ServerTransaction`] from the given request.
    ///
    /// # Panics
    ///
    /// Panics if request method is `ACK`.
    pub(crate) fn from_request(request: IncomingRequest, endpoint: Endpoint) -> Self {
        assert_ne!(
            request.req_line.method,
            SipMethod::Ack,
            "ACK requests do not create transactions"
        );

        let initial_state = if request.req_line.method == SipMethod::Invite {
            State::Proceeding
        } else {
            State::Trying
        };
        let state_machine = StateMachine::new(initial_state);
        let (sender, receiver) = mpsc::channel(10);

        let transaction_key = TransactionKey::from_request(&request);

        endpoint
            .transactions()
            .add_transaction(transaction_key.clone(), sender);

        Self {
            endpoint,
            transaction_key,
            request,
            state_machine,
            receiver: Some(receiver),
            target_info: None,
            provisonal_retrans_handle: None,
        }
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

        assert!(
            code.is_provisional(),
            "Invalid provisional response (expected 1xx) got {:?}",
            code
        );

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

        assert!(
            code.is_final(),
            "Invalid final response (expected 2xx-6xx) got {:?}",
            code
        );
        let is_invite_tsx = self.request.req_line.method == SipMethod::Invite;

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
            task.join_handle.await.unwrap()
        } else {
            self.receiver.take().unwrap()
        };

        if is_invite_tsx && !self.is_reliable() {
            tokio::spawn(async move {
                let mut retrans_interval = T1;
                loop {
                    let fut = time::timeout(retrans_interval, channel.recv());

                    match time::timeout_at(deadline, fut).await {
                        Ok(Ok(Some(TransactionMessage::Request(req))))
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

    #[cfg(test)]
    pub(crate) fn transaction_key(&self) -> &TransactionKey {
        &self.transaction_key
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
            .transport_info
            .transport
            .is_reliable()
    }

    fn spawn_retransmit_provisional_task(
        &mut self,
        mut response: OutgoingResponse,
    ) -> ProvisionalRetransHandle {
        let mut receiver = self.receiver.take().expect(
            "Transaction receiver missing while calling `spawn_retransmit_provisional_task`",
        );

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
    join_handle: task::JoinHandle<mpsc::Receiver<TransactionMessage>>,
    provisional_tx: mpsc::UnboundedSender<OutgoingResponse>,
}

impl Drop for ServerTransaction {
    fn drop(&mut self) {
        self.endpoint
            .transactions()
            .remove_transaction(&self.transaction_key);
    }
}

/// Unit tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_eq_state;
    use crate::test_utils::transaction::{
        CODE_100_TRYING, CODE_202_ACCEPTED, CODE_301_MOVED_PERMANENTLY, CODE_504_SERVER_TIMEOUT,
        ServerTestContext,
    };

    // INVITE Server tests

    #[tokio::test]
    async fn invite_transitions_to_proceeding_when_created_from_request() {
        let ctx = ServerTestContext::setup(SipMethod::Invite).await;

        assert_eq!(
            ctx.server.state_machine.state(),
            State::Proceeding,
            "server INVITE must transition to the Proceeding state when constructed for a request"
        );
    }

    #[tokio::test]
    async fn invite_transitions_to_confirmed_when_receiving_ack() {
        let mut ctx = ServerTestContext::setup(SipMethod::Invite).await;

        ctx.server
            .send_final_status(CODE_301_MOVED_PERMANENTLY)
            .await
            .expect("Error sending final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "server INVITE must transition to the Completed state when sending 300-699 response"
        );

        ctx.client.send_ack_request().await;

        assert_eq_state!(
            ctx.state,
            State::Confirmed,
            "server INVITE must transition to the Confirmed state when receiving the ACK request"
        );
    }

    #[tokio::test]
    async fn invite_unreliable_transitions_to_terminated_when_sending_2xx_response() {
        let mut ctx = ServerTestContext::setup(SipMethod::Invite).await;

        ctx.server
            .send_final_status(CODE_202_ACCEPTED)
            .await
            .expect("Error sending final response");

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "server INVITE must transition to the Terminated state when sending 2xx response"
        );
    }

    #[tokio::test]
    async fn invite_reliable_transitions_to_terminated_when_sending_2xx_response() {
        let mut ctx = ServerTestContext::setup_reliable(SipMethod::Invite).await;

        ctx.server
            .send_final_status(CODE_202_ACCEPTED)
            .await
            .expect("Error sending final response");

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "server INVITE must transition to the Terminated state when sending 2xx response"
        );
    }

    #[tokio::test]
    async fn invite_should_retransmit_response_when_receiving_request_retransmission() {
        let ctx = ServerTestContext::setup(SipMethod::Invite).await;
        let expected_responses = 1;
        let expected_retrans = 3;

        ctx.server
            .send_final_status(CODE_301_MOVED_PERMANENTLY)
            .await
            .expect("Error sending final response");

        ctx.client.retransmit_n_times(expected_retrans).await;

        assert_eq!(
            ctx.transport.sent_count(),
            expected_responses + expected_retrans,
            "sent count should match {expected_responses} responses and {expected_retrans} retransmissions"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn invite_must_cease_retransmission_when_receiving_ack() {
        let mut ctx = ServerTestContext::setup(SipMethod::Invite).await;
        let expected_responses = 1;
        let expected_retrans = 2;

        ctx.server
            .send_final_status(CODE_301_MOVED_PERMANENTLY)
            .await
            .expect("Error sending final response");

        ctx.timer.wait_for_retransmissions(expected_retrans).await;

        ctx.client.send_ack_request().await;

        // Should not retransmit at this point.
        ctx.timer.wait_for_retransmissions(3).await;

        assert_eq!(
            ctx.transport.sent_count(),
            expected_responses + expected_retrans,
            "sent count should match {expected_responses} responses and {expected_retrans} retransmissions"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn invite_timer_h_must_be_set_for_reliable_transports() {
        let mut ctx = ServerTestContext::setup_reliable(SipMethod::Invite).await;

        ctx.server
            .send_final_status(CODE_301_MOVED_PERMANENTLY)
            .await
            .expect("Error sending final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "server INVITE must transition to the Completed state when sending final 200-699 response"
        );

        ctx.timer.timer_h().await;

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "server INVITE must transition to the Terminated state when timer H fires"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn invite_timer_h_must_be_set_for_unreliable_transports() {
        let mut ctx = ServerTestContext::setup(SipMethod::Invite).await;

        ctx.server
            .send_final_status(CODE_301_MOVED_PERMANENTLY)
            .await
            .expect("Error sending final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "server INVITE must transition to the Completed state when sending 200-699 response"
        );

        ctx.timer.timer_h().await;

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "server INVITE must transition to the Terminated state when timer H fires"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn invite_transitions_to_terminated_when_timer_i_fires() {
        let mut ctx = ServerTestContext::setup(SipMethod::Invite).await;

        ctx.server
            .send_final_status(CODE_301_MOVED_PERMANENTLY)
            .await
            .expect("Error sending final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "server INVITE must must transition to the Completed when sending 300-699 response"
        );

        ctx.client.send_ack_request().await;

        assert_eq_state!(
            ctx.state,
            State::Confirmed,
            "server INVITE must transition to the Confirmed state when receiving ACK request"
        );

        ctx.timer.timer_i().await;

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "server INVITE must transition to the Terminated state when timer I fires"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn invite_retransmit_response_when_timer_g_fires() {
        let mut ctx = ServerTestContext::setup(SipMethod::Invite).await;
        let expected_responses = 1;
        let expected_retrans = 3;

        ctx.server
            .send_final_status(CODE_301_MOVED_PERMANENTLY)
            .await
            .expect("Error sending final response");

        ctx.timer.wait_for_retransmissions(expected_retrans).await;

        assert_eq!(
            ctx.transport.sent_count(),
            expected_responses + expected_retrans,
            "sent count should match {expected_responses} requests and {expected_retrans} retransmissions"
        );
    }

    // Non-INVITE Server tests

    #[tokio::test]
    async fn non_invite_transitions_to_trying_when_created_from_request() {
        let ctx = ServerTestContext::setup(SipMethod::Options).await;

        assert_eq!(
            ctx.server.state_machine.state(),
            State::Trying,
            "server non-INVITE must transition to the Trying state when constructed for a request"
        );
    }

    #[tokio::test]
    async fn non_invite_transition_to_proceeding_when_sending_1xx_response() {
        let mut ctx = ServerTestContext::setup(SipMethod::Options).await;

        ctx.server
            .send_provisional_status(CODE_100_TRYING)
            .await
            .expect("Error sending provisional response");

        assert_eq_state!(
            ctx.state,
            State::Proceeding,
            "server non-INVITE must transition to the Proceeding state when sending 1xx response"
        );
    }

    #[tokio::test]
    async fn non_invite_transition_to_completed_when_sending_non_2xx_response() {
        let mut ctx = ServerTestContext::setup(SipMethod::Options).await;

        ctx.server
            .send_final_status(CODE_504_SERVER_TIMEOUT)
            .await
            .expect("Error sending final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "server non-INVITE must transition to the Completed state when sending 200-699 response"
        );
    }

    #[tokio::test]
    async fn non_invite_reliable_transition_to_terminated_when_sending_2xx_response() {
        let mut ctx = ServerTestContext::setup_reliable(SipMethod::Options).await;

        ctx.server
            .send_final_status(CODE_202_ACCEPTED)
            .await
            .expect("Error sending final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "server non-INVITE must transition to the Completed when sending 200-699 response"
        );

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "server non-INVITE must transition to the Terminated state when sending 2xx response"
        );
    }

    #[tokio::test]
    async fn non_invite_reliable_transition_to_terminated_when_sending_non_2xx_response() {
        let mut ctx = ServerTestContext::setup_reliable(SipMethod::Options).await;

        ctx.server
            .send_final_status(CODE_504_SERVER_TIMEOUT)
            .await
            .expect("Error sending final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "server non-INVITE must transition to the Completed when sending 200-699 response"
        );

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "server non-INVITE must transition to the Terminated state when sending 2xx response"
        );
    }

    #[tokio::test]
    async fn non_invite_absorbs_retransmission_in_trying_state() {
        let ctx = ServerTestContext::setup(SipMethod::Options).await;
        let expected_retrans = 0;

        ctx.client.retransmit_n_times(2).await;

        assert_eq!(
            ctx.transport.sent_count(),
            expected_retrans,
            "sent count should match {expected_retrans} retransmissions"
        );
    }

    #[tokio::test]
    async fn non_invite_retransmit_provisional_response_when_receiving_request_retransmission() {
        let mut ctx = ServerTestContext::setup(SipMethod::Options).await;
        let expected_responses = 1;
        let expected_retrans = 4;

        ctx.server
            .send_provisional_status(CODE_100_TRYING)
            .await
            .expect("Error sending provisional response");

        ctx.client.retransmit_n_times(expected_retrans).await;

        assert_eq!(
            ctx.transport.sent_count(),
            expected_responses + expected_retrans,
            "sent count should match {expected_responses} responses and {expected_retrans} retransmissions"
        );
    }

    #[tokio::test]
    async fn non_invite_retransmit_final_response_when_receiving_request_retransmission() {
        let ctx = ServerTestContext::setup(SipMethod::Register).await;
        let expected_responses = 1;
        let expected_retrans = 2;

        ctx.server
            .send_final_status(CODE_202_ACCEPTED)
            .await
            .expect("Error sending final response");

        ctx.client.retransmit_n_times(expected_retrans).await;

        assert_eq!(
            ctx.transport.sent_count(),
            expected_responses + expected_retrans,
            "sent count should match {expected_responses} responses and {expected_retrans} retransmissions"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn non_invite_transitions_to_terminated_when_timer_j_fires() {
        let mut ctx = ServerTestContext::setup(SipMethod::Bye).await;

        ctx.server
            .send_final_status(CODE_202_ACCEPTED)
            .await
            .expect("Error sending final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "server non-INVITE must must transition to the Completed state when sending 200-699 response"
        );

        ctx.timer.timer_j().await;

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "server non-INVITE must transition to the Terminated state when timer J fires"
        );
    }
}
