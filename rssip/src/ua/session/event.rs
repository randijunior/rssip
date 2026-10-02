use crate::transport::incoming::IncomingRequest;
use media::sdp::SessionDescription;

pub trait SessionEventHandler: Send + Sync + 'static {
    fn on_reinvite(&self, _reinvite: IncomingRequest) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn on_terminated(&self, _evt: SessionTerminatedEvent) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn on_negotiation_done(
        &self,
        _evt: SessionNegotiationDoneEvent,
    ) -> impl Future<Output = ()> + Send {
        async {}
    }
}

#[derive(Debug)]
pub struct SessionTerminatedEvent {
    pub cause: TerminatedCause,
}

#[derive(Debug, Clone, Copy)]
pub enum TerminatedCause {
    ByeReceived,
}

pub struct SessionNegotiationDoneEvent<'a> {
    pub local_sdp: &'a SessionDescription,
    pub remote_sdp: &'a SessionDescription,
    pub answer: &'a SessionDescription,
}
