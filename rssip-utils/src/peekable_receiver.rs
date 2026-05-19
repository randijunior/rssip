use tokio::sync::mpsc;

pub struct PeekableReceiver<T> {
    rx: mpsc::Receiver<T>,
    peeked: Option<T>,
}

impl<T> From<mpsc::Receiver<T>> for PeekableReceiver<T> {
    fn from(rx: mpsc::Receiver<T>) -> Self {
        Self::new(rx)
    }
}

impl<T> PeekableReceiver<T> {
    pub fn new(rx: mpsc::Receiver<T>) -> Self {
        Self { rx, peeked: None }
    }

    pub async fn recv(&mut self) -> Option<T> {
        match self.peeked.take() {
            Some(msg) => Some(msg),
            None => self.rx.recv().await,
        }
    }
    pub fn try_recv(&mut self) -> Result<T, mpsc::error::TryRecvError> {
        match self.peeked.take() {
            Some(msg) => Ok(msg),
            None => self.rx.try_recv(),
        }
    }
    pub async fn peek(&mut self) -> Option<&T> {
        if self.peeked.is_none() {
            self.peeked = self.rx.recv().await;
        }
        self.peeked.as_ref()
    }

    pub async fn recv_if(&mut self, cond: impl FnOnce(&T) -> bool) -> Option<T> {
        match self.peek().await {
            Some(matched) if cond(matched) => self.peeked.take(),
            _ => None,
        }
    }
}
