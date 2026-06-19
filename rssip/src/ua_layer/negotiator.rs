use sdp::msg::SessionDescription;

use crate::error::Result;

// RFC 3264: An Offer/Answer Model with the Session Description Protocol (SDP)

#[derive(Default)]
pub struct Negotiator<N> {
    neg: N,
}

pub struct RemoteOffer {
    remote: SessionDescription,
}

pub struct LocalOffer {
    local: SessionDescription,
}

pub struct WaitNego {
    local: LocalOffer,
    remote: RemoteOffer,
}

struct Negotiated {
    negotiated: SessionDescription,
}

pub struct Done {
    local: LocalOffer,
    remote: RemoteOffer,

    negotiated: Negotiated,
}

impl LocalOffer {
    pub fn new(local: SessionDescription) -> Self {
        Self { local }
    }
}

impl RemoteOffer {
    pub fn new(remote: SessionDescription) -> Self {
        Self { remote }
    }
}

impl Negotiator<()> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Negotiator<RemoteOffer> {
    // Early Offer
    pub fn from_remote(remote: RemoteOffer) -> Self {
        todo!()
    }
    pub fn set_local_offer(self, local: LocalOffer) -> Negotiator<WaitNego> {
        todo!()
    }
}

impl Negotiator<LocalOffer> {
    // Late Offer
    pub fn from_local(local: LocalOffer) -> Self {
        Self { neg: local }
    }
    pub fn process_answer(self, sdp: SessionDescription) -> Result<Negotiator<Done>> {
        todo!()
    }
}

impl Negotiator<WaitNego> {
    pub fn negotiate(self) -> Result<Negotiator<Done>> {
        todo!()
    }
}
