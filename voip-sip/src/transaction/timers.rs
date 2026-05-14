use std::time::Duration;

/// Estimated round‑trip time (RTT) for message exchanges.
pub(crate) const T1: Duration = Duration::from_millis(500);

/// Maximum retransmission interval for non‑INVITE requests and INVITE responses.
pub(crate) const T2: Duration = Duration::from_secs(4);

/// Maximum duration that a message may remain in the network before being discarded.
pub(crate) const T4: Duration = Duration::from_secs(5);
