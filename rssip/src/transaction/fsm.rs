/// Defines the possible states of a SIP Transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    /// Calling state
    Calling,
    /// Trying state
    Trying,
    /// Proceeding state
    Proceeding,
    /// Completed state
    Completed,
    /// Confirmed state
    Confirmed,
    /// Terminated state
    Terminated,
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state_str = match self {
            Self::Calling => "Calling",
            Self::Trying => "Trying",
            Self::Proceeding => "Proceeding",
            Self::Completed => "Completed",
            Self::Confirmed => "Confirmed",
            Self::Terminated => "Terminated",
        };
        write!(f, "{}", state_str)
    }
}

use tokio::sync::broadcast;

pub struct StateMachine {
    state: State,
    state_change_notifier: Option<broadcast::Sender<State>>,
}

impl StateMachine {
    pub fn new(state: State) -> Self {
        Self {
            state,
            state_change_notifier: None,
        }
    }
    /// Subscribe to transaction state changes
    ///
    /// Returns a broadcast::Receiver that can be used to monitor state changes
    pub fn subscribe_state(&mut self) -> TsxStateChangeReceiver {
        let recv = match self.state_change_notifier {
            Some(ref state) => state.subscribe(),
            None => {
                let (sender, recv) = broadcast::channel(6);

                self.state_change_notifier = Some(sender);

                recv
            }
        };

        TsxStateChangeReceiver(recv)
    }

    #[must_use]
    fn borrow_state_notifier(&self) -> Option<&broadcast::Sender<State>> {
        self.state_change_notifier.as_ref()
    }

    fn notify_state_change(&self, state: State) {
        if let Some(sender) = self.borrow_state_notifier() {
            let _result = sender.send(state);
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn set_state(&mut self, state: State) {
        self.state = state;

        self.notify_state_change(state);
    }
}

pub struct TsxStateChangeReceiver(broadcast::Receiver<State>);

impl TsxStateChangeReceiver {
    pub async fn recv(&mut self) -> crate::Result<State> {
        self.0.recv().await.map_err(|_| crate::Error::ChannelClosed)
    }
}
