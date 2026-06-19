use std::time::Duration;

/// Estimated round‑trip time (RTT) for message exchanges.
pub(crate) const T1: Duration = Duration::from_millis(500);

/// Maximum retransmission interval for non‑INVITE requests and INVITE responses.
pub(crate) const T2: Duration = Duration::from_secs(4);

/// Maximum duration that a message may remain in the network before being discarded.
pub(crate) const T4: Duration = Duration::from_secs(5);

/// A mock timer, for testing purposes
#[cfg(test)]
pub(crate) struct MockRetransTimer(Duration);

#[cfg(test)]
impl MockRetransTimer {
    pub fn new() -> Self {
        Self(T1)
    }

    fn set_next_interval(&mut self) {
        self.0 = std::cmp::min(self.0 * 2, T2);
    }

    pub async fn wait_interval(&self) {
        tokio::time::sleep(self.0).await;
    }

    pub async fn wait_for_retransmissions(&mut self, n: usize) {
        for _ in 0..n {
            self.wait_interval().await;
            self.set_next_interval();
            tokio::task::yield_now().await;
        }
    }
}
