/// Unit tests

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
use crate::transport::TransportHandle;
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
        target: (TransportHandle, SocketAddr),
    ) -> Result<Self> {
        Self::send(request, endpoint, Some(target)).await
    }

    async fn send(
        request: Request,
        endpoint: Endpoint,
        target: Option<(TransportHandle, SocketAddr)>,
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
