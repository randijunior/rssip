use crate::codec::Codec;
use crate::error::{Error, Result};
use crate::sdp::SessionDescription;

// RFC 3264: An Offer/Answer Model with the Session Description Protocol (SDP)
#[derive(Default)]
pub struct Negotiator {
    remote_offer: Option<SessionDescription>,
    local_offer: Option<SessionDescription>,
    state: NegoState,
}

#[derive(Default, Debug, PartialEq, Eq)]
enum NegoState {
    #[default]
    Init,
    RemoteOffer,
    LocalOffer,
    Negotiating,
    Done,
}

impl Negotiator {
    pub fn add_codec() {}
    pub fn create_offer() {}

    pub fn from_remote(remote: SessionDescription) -> Self {
        Self {
            remote_offer: Some(remote),
            local_offer: None,
            state: NegoState::RemoteOffer,
        }
    }

    pub fn is_negotiating(&self) -> bool {
        self.state == NegoState::Negotiating
    }

    pub fn set_remote_sdp(&mut self, remote: SessionDescription) -> Result<()> {
        self.state = match self.state {
            NegoState::Init => NegoState::RemoteOffer,
            NegoState::LocalOffer => NegoState::Negotiating,
            _ => return Err(Error::InvalidNegoStateError),
        };
        self.remote_offer = Some(remote);
        Ok(())
    }

    pub fn set_local_sdp(&mut self, local: SessionDescription) -> Result<()> {
        self.state = match self.state {
            NegoState::Init => NegoState::LocalOffer,
            NegoState::RemoteOffer => NegoState::Negotiating,
            _ => return Err(Error::InvalidNegoStateError),
        };
        self.local_offer = Some(local);
        Ok(())
    }

    pub fn generate_answer(&self) -> Result<()> {
        todo!()
    }
}
