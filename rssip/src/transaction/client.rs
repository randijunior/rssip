use std::net::SocketAddr;

use tokio::sync::mpsc::{self};
use tokio::time::{self, Instant};
use utils::PeekableReceiver;

use crate::error::{Error, TransactionError};
use crate::message::Request;
use crate::message::headers::via::Rport;
use crate::message::headers::{Header, Via};
use crate::message::method::SipMethod;
use crate::transaction::Role;
use crate::transaction::fsm::{State, StateMachine};
use crate::transaction::manager::TransactionKey;
use crate::transaction::timers::{T1, T4};
use crate::transport::Transport;
use crate::transport::incoming::{IncomingMessage, IncomingResponse};
use crate::transport::outgoing::OutgoingRequest;
use crate::{Endpoint, Result, find_map_mut_header};

/// A Client Transaction.
///
/// Represents a SIP client transaction.
pub struct ClientTransaction {
    key: TransactionKey,
    endpoint: Endpoint,
    state_machine: StateMachine,
    request: OutgoingRequest,
    channel: PeekableReceiver<IncomingMessage>,
    timeout: Instant,
}

impl ClientTransaction {
    pub(crate) async fn send_request(request: Request, endpoint: Endpoint) -> Result<Self> {
        Self::send(request, endpoint, None).await
    }

    pub(crate) async fn send_request_with_target(
        request: Request,
        endpoint: Endpoint,
        target: (Transport, SocketAddr),
    ) -> Result<Self> {
        Self::send(request, endpoint, Some(target)).await
    }

    async fn send(
        request: Request,
        endpoint: Endpoint,
        target: Option<(Transport, SocketAddr)>,
    ) -> Result<Self> {
        let method = request.req_line.method;
        assert_ne!(
            method,
            SipMethod::Ack,
            "ACK requests do not create transactions"
        );
        let mut outgoing = endpoint.create_outgoing_request(request, target).await?;
        let headers = &mut outgoing.request.headers;

        let via = match find_map_mut_header!(headers, Via) {
            Some(via) => via,
            None => {
                let sent_by = outgoing.target_info.transport.local_addr().into();
                let transport = outgoing.target_info.transport.protocol();
                let branch = crate::generate_branch();
                let via =
                    Via::new_with_transport(transport, sent_by, Some(branch), Some(Rport(None)));

                let header = headers.insert_mut(0, Header::Via(via));

                header
                    .as_via_mut()
                    .expect("The 'insert_mut' will return the same 'Via' header")
            }
        };
        let branch = match via.branch.as_ref().map(|b| b.to_owned()) {
            Some(branch) => branch,
            None => {
                let branch = crate::generate_branch();
                via.branch = Some(branch.clone());
                branch
            }
        };
        let key = TransactionKey::new_key_3261(Role::Uac, method, branch);

        endpoint.send_outgoing_request(&mut outgoing).await?;

        let state = if method == SipMethod::Invite {
            State::Calling
        } else {
            State::Trying
        };
        let (sender, channel) = mpsc::channel(10);

        endpoint.tsx_plugin().add_transaction(key.clone(), sender);

        let client_tsx = Self {
            key,
            endpoint,
            state_machine: StateMachine::new(state),
            channel: channel.into(),
            request: outgoing,
            timeout: Instant::now() + T1 * 64,
        };

        log::trace!(
            "Client Transaction Created [{:#?}] ({:p})",
            method,
            &client_tsx
        );

        Ok(client_tsx)
    }

    pub async fn receive_provisional_response(&mut self) -> Result<Option<IncomingResponse>> {
        let state = self.state();

        if state <= State::Proceeding {
            let response = self.receive_provisional().await?;

            if state != State::Proceeding {
                self.state_machine.set_state(State::Proceeding);
            }

            Ok(response)
        } else {
            Ok(None)
        }
    }

    pub async fn receive_final_response(mut self) -> Result<IncomingResponse> {
        let response = self.receive_final().await?;

        let is_invite_tsx = self.request.req_line.method == SipMethod::Invite;

        if self.is_reliable()
            || (is_invite_tsx
                && matches!(response.status_line.code.as_u16(), 200..299)
                && matches!(self.state(), State::Calling | State::Proceeding))
        {
            self.state_machine.set_state(State::Terminated);
            return Ok(response);
        }

        self.state_machine.set_state(State::Completed);

        let (timer, mut ack_request) = if is_invite_tsx {
            let mut ack_request = self.endpoint.create_ack_request(&self.request, &response);
            self.endpoint
                .send_outgoing_request(&mut ack_request)
                .await?;

            (Instant::now() + 64 * T1, Some(ack_request))
        } else {
            (Instant::now() + T4, None)
        };

        tokio::spawn(async move {
            while let Ok(Some(IncomingMessage::Response(_))) =
                time::timeout_at(timer, self.channel.recv()).await
            {
                if let Some(ref mut ack) = ack_request
                    && let Err(err) = self.endpoint.send_outgoing_request(ack).await
                {
                    log::error!("Failed to retransmit ack: {}", err);
                }
            }
            self.state_machine.set_state(State::Terminated);
        });

        Ok(response)
    }

    async fn receive_provisional(&mut self) -> Result<Option<IncomingResponse>> {
        self.receive_response_if(|res| res.status_line.code.is_provisional())
            .await
    }

    async fn receive_final(&mut self) -> Result<IncomingResponse> {
        self.receive_response_if(|res| res.status_line.code.is_final())
            .await?
            .ok_or(Error::ChannelClosed)
    }

    async fn receive_response_if(
        &mut self,
        pred: fn(&IncomingResponse) -> bool,
    ) -> Result<Option<IncomingResponse>> {
        let deadline = self.timeout;

        match self.state() {
            State::Calling | State::Trying if self.is_unreliable() => {
                let mut retrans_interval = T1;
                loop {
                    let fut = time::timeout(retrans_interval, self.recv_if(pred));

                    match time::timeout_at(deadline, fut).await {
                        Ok(Ok(Some(msg))) => return Ok(Some(msg)),
                        Ok(Err(_elapsed)) => {
                            // retransmit
                            self.endpoint
                                .send_outgoing_request(&mut self.request)
                                .await?;
                            retrans_interval *= 2;
                            continue;
                        }
                        Err(_elapsed) => {
                            self.state_machine.set_state(State::Terminated);
                            return Err(TransactionError::Timeout.into());
                        }
                        Ok(Ok(None)) => return Ok(None),
                    }
                }
            }
            State::Calling | State::Trying => {
                match time::timeout_at(deadline, self.recv_if(pred)).await {
                    Ok(Some(msg)) => Ok(Some(msg)),
                    Err(_elapsed) => {
                        self.state_machine.set_state(State::Terminated);
                        Err(TransactionError::Timeout.into())
                    }
                    Ok(None) => Ok(None),
                }
            }
            State::Proceeding if self.request.req_line.method == SipMethod::Invite => {
                match self.recv_if(pred).await {
                    Some(msg) => Ok(Some(msg)),
                    _ => Ok(None),
                }
            }
            State::Proceeding => match time::timeout_at(deadline, self.recv_if(pred)).await {
                Ok(Some(msg)) => Ok(Some(msg)),
                Err(_elapsed) => {
                    self.state_machine.set_state(State::Terminated);
                    Err(TransactionError::Timeout.into())
                }
                Ok(None) => Ok(None),
            },
            _ => unreachable!(),
        }
    }

    async fn recv_if(&mut self, cond: fn(&IncomingResponse) -> bool) -> Option<IncomingResponse> {
        let cond = |msg: &IncomingMessage| matches!(msg, IncomingMessage::Response(response) if cond(response));

        match self.channel.recv_if(cond).await {
            Some(IncomingMessage::Response(res)) => Some(res),
            _ => None,
        }
    }

    pub(crate) fn state(&self) -> State {
        self.state_machine.state()
    }

    pub fn state_machine_mut(&mut self) -> &mut StateMachine {
        &mut self.state_machine
    }

    pub fn key(&self) -> &TransactionKey {
        &self.key
    }

    fn is_reliable(&self) -> bool {
        self.request.target_info.transport.is_reliable()
    }

    fn is_unreliable(&self) -> bool {
        self.request.target_info.transport.is_unreliable()
    }
}

impl Drop for ClientTransaction {
    fn drop(&mut self) {
        self.endpoint.tsx_plugin().remove_transaction(&self.key);
        log::trace!("Transaction Destroyed [{:#?}] ({:p})", Role::Uac, &self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_eq_state;
    use crate::error::{Error, TransactionError};
    use crate::test_utils::transaction::{
        CODE_100_TRYING, CODE_180_RINGING, CODE_202_ACCEPTED, CODE_301_MOVED_PERMANENTLY,
        CODE_404_NOT_FOUND, CODE_504_SERVER_TIMEOUT, CODE_603_DECLINE, ClientTestContext,
        SendRequestContext,
    };

    // INVITE Client tests

    #[tokio::test]
    async fn invite_transitions_to_calling_when_request_is_sent() {
        let ctx = SendRequestContext::setup(SipMethod::Invite).await;

        let uac = ClientTransaction::send_request_with_target(
            ctx.request,
            ctx.endpoint,
            (ctx.transport, ctx.destination),
        )
        .await
        .expect("error sending request");

        assert_eq!(
            uac.state(),
            State::Calling,
            "client INVITE must transition to the Calling state when sending the request"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn invite_should_not_start_timer_a_when_transport_is_reliable() {
        let mut ctx = ClientTestContext::setup_reliable(SipMethod::Invite).await;
        let expected_requests = 1;
        let expected_retrans = 0;

        let opt_err = ctx.client.receive_provisional_response().await.err();

        assert_matches!(
            opt_err,
            Some(Error::TransactionError(TransactionError::Timeout)),
            "Expected TransactionError::Timeout, got {opt_err:?}"
        );

        assert_eq!(
            ctx.transport.sent_count(),
            expected_requests + expected_retrans,
            "sent count should match {expected_requests} requests and {expected_retrans} retransmissions"
        );
    }

    #[tokio::test]
    async fn invite_transitions_from_calling_to_proceeding_when_receiving_1xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_100_TRYING).await;

        ctx.client
            .receive_provisional_response()
            .await
            .expect("Error receiving provisional response")
            .expect("Expected provisional response, but received None");

        assert_eq_state!(
            ctx.state,
            State::Proceeding,
            "client INVITE must transition to the Proceeding state when receiving 1xx response"
        );
    }

    #[tokio::test]
    async fn invite_transitions_from_calling_to_completed_when_receiving_3xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client INVITE must transition to the Completed state when receiving 3xx response"
        );
    }

    #[tokio::test]
    async fn invite_transitions_from_calling_to_completed_when_receiving_4xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_404_NOT_FOUND).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client INVITE must transition to the Completed state when receiving 4xx response"
        );
    }

    #[tokio::test]
    async fn invite_transitions_from_calling_to_completed_when_receiving_5xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_504_SERVER_TIMEOUT).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client INVITE must transition to the Completed state when receiving 5xx response"
        );
    }

    #[tokio::test]
    async fn invite_transitions_from_calling_to_completed_when_receiving_6xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_603_DECLINE).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client INVITE must transition to the Completed state when receiving 6xx response"
        );
    }

    #[tokio::test]
    async fn invite_transitions_from_calling_to_terminated_when_receiving_2xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_202_ACCEPTED).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "client INVITE must transition to the Terminated state when receiving 2xx response"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn invite_transitions_from_calling_to_terminated_when_timer_b_fires() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        let opt_err = ctx.client.receive_provisional_response().await.err();

        assert_matches!(
            opt_err,
            Some(Error::TransactionError(TransactionError::Timeout)),
            "Expected TransactionError::Timeout, got {opt_err:?}"
        );

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "client INVITE must transition to the Terminated state when timer B fires"
        );
    }

    #[tokio::test]
    async fn invite_should_send_ack_when_receiving_3xx_response_in_calling_state() {
        let ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        let req = ctx.transport.get_last_sent_request().expect("A request");
        assert_eq!(
            req.req_line.method,
            SipMethod::Ack,
            "client INVITE must generate an ACK request when receiving 3xx response"
        );
    }

    #[tokio::test]
    async fn invite_should_send_ack_when_receiving_4xx_response_in_calling_state() {
        let ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_404_NOT_FOUND).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        let req = ctx.transport.get_last_sent_request().expect("A request");
        assert_eq!(
            req.req_line.method,
            SipMethod::Ack,
            "client INVITE must generate an ACK request when receiving 4xx response"
        );
    }

    #[tokio::test]
    async fn invite_should_send_ack_when_receiving_5xx_response_in_calling_state() {
        let ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_504_SERVER_TIMEOUT).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        let req = ctx.transport.get_last_sent_request().expect("A request");
        assert_eq!(
            req.req_line.method,
            SipMethod::Ack,
            "client INVITE must generate an ACK request when receiving 5xx response"
        );
    }

    #[tokio::test]
    async fn invite_should_send_ack_when_receiving_6xx_response_in_calling_state() {
        let ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_603_DECLINE).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        let req = ctx.transport.get_last_sent_request().expect("A request");
        assert_eq!(
            req.req_line.method,
            SipMethod::Ack,
            "client INVITE must generate an ACK request when receiving 6xx response"
        );
    }

    #[tokio::test]
    async fn invite_transitions_from_proceeding_to_completed_when_receiving_3xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client INVITE must transition to the Completed state when receiving 3xx response"
        );
    }

    #[tokio::test]
    async fn invite_transitions_from_proceeding_to_completed_when_receiving_4xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_404_NOT_FOUND).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client INVITE must transition to the Completed state when receiving 4xx response"
        );
    }

    #[tokio::test]
    async fn invite_transitions_from_proceeding_to_completed_when_receiving_5xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_504_SERVER_TIMEOUT).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client INVITE must transition to the Completed state when receiving 5xx response"
        );
    }

    #[tokio::test]
    async fn invite_transitions_from_proceeding_to_completed_when_receiving_6xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_603_DECLINE).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client INVITE must transition to the Completed state when receiving 6xx response"
        );
    }

    #[tokio::test]
    async fn invite_transitions_from_proceeding_to_terminated_when_receiving_2xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_202_ACCEPTED).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "client INVITE must transition to the Completed state when receiving receiving 2xx response"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn invite_should_not_retransmit_request_in_proceeding_state() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;
        let expected_requests = 1;
        let expected_retrans = 0;

        ctx.server.respond(CODE_100_TRYING).await;

        ctx.client
            .receive_provisional_response()
            .await
            .expect("Error receiving provisional response")
            .expect("Expected provisional response, but received None");

        assert_eq_state!(
            ctx.state,
            State::Proceeding,
            "client INVITE must transition to the Proceeding state when receiving receiving 1xx response"
        );

        ctx.timer.wait_for_retransmissions(5).await;

        assert_eq!(
            ctx.transport.sent_count(),
            expected_requests + expected_retrans
        );
    }

    #[tokio::test]
    async fn invite_should_send_ack_when_receiving_3xx_response_in_proceeding_state() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_100_TRYING).await;

        ctx.client
            .receive_provisional_response()
            .await
            .expect("Error receiving provisional response")
            .expect("Expected provisional response, but received None");

        assert_eq_state!(
            ctx.state,
            State::Proceeding,
            "client INVITE must transition to the Proceeding state when receiving receiving 1xx response"
        );

        ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        let req = ctx.transport.get_last_sent_request().expect("A request");
        assert_eq!(
            req.req_line.method,
            SipMethod::Ack,
            "client INVITE must generate an ACK request when receiving 3xx response"
        );
    }

    #[tokio::test]
    async fn invite_should_send_ack_after_4xx_response_in_proceeding_state() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_100_TRYING).await;

        ctx.client
            .receive_provisional_response()
            .await
            .expect("Error receiving provisional response")
            .expect("Expected provisional response, but received None");

        assert_eq_state!(
            ctx.state,
            State::Proceeding,
            "client INVITE must transition to the Proceeding state when receiving receiving 1xx response"
        );

        ctx.server.respond(CODE_404_NOT_FOUND).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        let req = ctx.transport.get_last_sent_request().expect("A request");
        assert_eq!(
            req.req_line.method,
            SipMethod::Ack,
            "client INVITE must generate an ACK request when receiving 4xx response"
        );
    }

    #[tokio::test]
    async fn invite_should_send_ack_when_receiving_5xx_response_in_proceeding_state() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_100_TRYING).await;

        ctx.client
            .receive_provisional_response()
            .await
            .expect("Error receiving provisional response")
            .expect("Expected provisional response, but received None");

        assert_eq_state!(
            ctx.state,
            State::Proceeding,
            "client INVITE must transition to the Proceeding state when receiving receiving 1xx response"
        );

        ctx.server.respond(CODE_504_SERVER_TIMEOUT).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        let req = ctx.transport.get_last_sent_request().expect("A request");
        assert_eq!(
            req.req_line.method,
            SipMethod::Ack,
            "client INVITE must generate an ACK request when receiving 5xx response"
        );
    }

    #[tokio::test]
    async fn invite_should_send_ack_after_6xx_response_in_proceeding_state() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_100_TRYING).await;

        ctx.client
            .receive_provisional_response()
            .await
            .expect("Error receiving provisional response")
            .expect("Expected provisional response, but received None");

        assert_eq_state!(
            ctx.state,
            State::Proceeding,
            "client INVITE must transition to the Proceeding state when receiving receiving 1xx response"
        );

        ctx.server.respond(CODE_603_DECLINE).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        let req = ctx.transport.get_last_sent_request().expect("A request");
        assert_eq!(
            req.req_line.method,
            SipMethod::Ack,
            "client INVITE must generate an ACK request when receiving 6xx response"
        );
    }

    #[tokio::test]
    async fn invite_should_pass_provisional_responses_to_tu_in_proceeding_state() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_100_TRYING).await;
        ctx.server.respond(CODE_180_RINGING).await;

        let incoming = ctx
            .client
            .receive_provisional_response()
            .await
            .expect("Error receiving provisional response")
            .expect("Expected provisional response, but received None");

        assert_eq!(
            incoming.response.status_line.code, CODE_100_TRYING,
            "should match 100 status code"
        );

        let incoming = ctx
            .client
            .receive_provisional_response()
            .await
            .expect("Error receiving provisional response")
            .expect("Expected provisional response, but received None");

        assert_eq!(
            incoming.response.status_line.code, CODE_180_RINGING,
            "should match 180 status code"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn invite_transitions_from_completed_to_terminated_when_timer_d_fires() {
        let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

        ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client INVITE must transition to the Completed state when receiving 3xx response"
        );

        ctx.timer.timer_d().await;

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "client INVITE must transition to the Terminated state when timer D fires"
        );
    }

    // Non-INVITE Client tests

    #[tokio::test]
    async fn non_invite_transitions_to_trying_when_request_is_sent() {
        let ctx = SendRequestContext::setup(SipMethod::Register).await;

        let uac = ClientTransaction::send_request_with_target(
            ctx.request,
            ctx.endpoint,
            (ctx.transport, ctx.destination),
        )
        .await
        .expect("failure sending request");

        assert_eq!(
            uac.state(),
            State::Trying,
            "client non-INVITE must transition to the Trying state when sending the request"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn non_invite_should_not_start_timer_e_when_transport_is_reliable() {
        let mut ctx = ClientTestContext::setup_reliable(SipMethod::Invite).await;
        let expected_requests = 1;
        let expected_retrans = 0;

        let opt_err = ctx.client.receive_provisional_response().await.err();

        assert_matches!(
            opt_err,
            Some(Error::TransactionError(TransactionError::Timeout)),
            "Expected TransactionError::Timeout, got {opt_err:?}"
        );

        assert_eq!(
            ctx.transport.sent_count(),
            expected_requests + expected_retrans,
            "sent count should match {expected_requests} requests and {expected_retrans} retransmissions"
        );
    }

    #[tokio::test]
    async fn non_invite_transitions_from_trying_to_proceeding_when_receiving_1xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Register).await;

        ctx.server.respond(CODE_100_TRYING).await;

        ctx.client
            .receive_provisional_response()
            .await
            .expect("Error receiving provisional response")
            .expect("Expected provisional response, but received None");

        assert_eq_state!(
            ctx.state,
            State::Proceeding,
            "client non-INVITE must transition to the Proceeding state when receiving receiving 1xx response"
        );
    }

    #[tokio::test]
    async fn non_invite_transitions_from_trying_to_completed_when_receiving_2xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

        ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client non-INVITE must transition to the Completed state when receiving receiving 6xx response"
        );
    }

    #[tokio::test]
    async fn non_invite_transitions_from_trying_to_completed_when_receiving_3xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

        ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client non-INVITE must transition to the Completed state when receiving receiving 3xx response"
        );
    }

    #[tokio::test]
    async fn non_invite_transitions_from_trying_to_completed_when_receiving_4xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

        ctx.server.respond(CODE_404_NOT_FOUND).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client non-INVITE must transition to the Completed state when receiving receiving 4xx response"
        );
    }

    #[tokio::test]
    async fn non_invite_transitions_from_trying_to_completed_when_receiving_5xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

        ctx.server.respond(CODE_504_SERVER_TIMEOUT).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client non-INVITE must transition to the Completed state when receiving receiving 5xx response"
        );
    }

    #[tokio::test]
    async fn non_invite_transitions_from_trying_to_completed_when_receiving_6xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

        ctx.server.respond(CODE_603_DECLINE).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client non-INVITE must transition to the Completed state when receiving receiving 6xx response"
        );
    }
    #[tokio::test]
    async fn non_invite_transitions_from_proceeding_to_completed_when_receiving_3xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

        ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client non-INVITE must transition to the Completed state when receiving receiving 3xx response"
        );
    }

    #[tokio::test]
    async fn non_invite_transitions_from_proceeding_to_completed_when_receiving_4xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

        ctx.server.respond(CODE_404_NOT_FOUND).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client non-INVITE must transition to the Completed state when receiving receiving 4xx response"
        );
    }

    #[tokio::test]
    async fn non_invite_transitions_from_proceeding_to_completed_when_receiving_5xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

        ctx.server.respond(CODE_504_SERVER_TIMEOUT).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client non-INVITE must transition to the Completed state when receiving receiving 5xx response"
        );
    }

    #[tokio::test]
    async fn non_invite_transitions_from_proceeding_to_completed_when_receiving_6xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

        ctx.server.respond(CODE_603_DECLINE).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client non-INVITE must transition to the Completed state when receiving receiving 6xx response"
        );
    }

    #[tokio::test]
    async fn non_invite_transitions_from_proceeding_to_completed_when_receiving_2xx_response() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

        ctx.server.respond(CODE_202_ACCEPTED).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client non-INVITE must transition to the Completed state when receiving receiving 2xx response"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn non_invite_transitions_from_trying_to_terminated_when_timer_f_fires() {
        let mut ctx = ClientTestContext::setup(SipMethod::Register).await;

        let opt_err = ctx.client.receive_provisional_response().await.err();

        assert_matches!(
            opt_err,
            Some(Error::TransactionError(TransactionError::Timeout)),
            "Expected TransactionError::Timeout, got {opt_err:?}"
        );

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "client non-INVITE must transition to the Terminated state when timer F fires"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn non_invite_should_not_retransmit_request_in_proceeding_state() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;
        let expected_requests = 1;
        let expected_retrans = 0;

        ctx.server.respond(CODE_100_TRYING).await;

        ctx.client
            .receive_provisional_response()
            .await
            .expect("Error receiving provisional response")
            .expect("Expected provisional response, but received None");

        assert_eq_state!(
            ctx.state,
            State::Proceeding,
            "client non-INVITE must transition to the Proceeding state when receiving receiving 1xx response"
        );

        ctx.timer.wait_for_retransmissions(5).await;

        assert_eq!(
            ctx.transport.sent_count(),
            expected_requests + expected_retrans,
            "sent count should match {expected_requests} requests and {expected_retrans} retransmissions"
        );
    }

    #[tokio::test]
    async fn non_invite_should_pass_provisional_responses_to_tu_in_proceeding_state() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

        ctx.server.respond(CODE_100_TRYING).await;
        ctx.server.respond(CODE_180_RINGING).await;

        let response = ctx
            .client
            .receive_provisional_response()
            .await
            .expect("Error receiving provisional response")
            .expect("Expected provisional response, but received None");

        assert_eq!(
            response.response.status_line.code, CODE_100_TRYING,
            "should match 100 status code"
        );

        let response = ctx
            .client
            .receive_provisional_response()
            .await
            .expect("Error receiving provisional response")
            .expect("Expected provisional response, but received None");

        assert_eq!(
            response.response.status_line.code, CODE_180_RINGING,
            "should match 180 status code"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn non_invite_transitions_from_completed_to_terminated_when_timer_k_fires() {
        let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

        ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

        ctx.client
            .receive_final_response()
            .await
            .expect("Error receiving final response");

        assert_eq_state!(
            ctx.state,
            State::Completed,
            "client non-INVITE must transition to the Completed state when receiving receiving 3xx response"
        );

        ctx.timer.timer_k().await;
        tokio::task::yield_now().await;

        assert_eq_state!(
            ctx.state,
            State::Terminated,
            "should transition to Terminated after timer d fires"
        );
    }
}
