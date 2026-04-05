use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures::Stream;
use parking_lot::Mutex;
use tokio::sync::watch;

#[derive(Debug)]
pub struct EventStream<T, R> {
    receiver: tokio::sync::mpsc::Receiver<StreamEvent<T>>,
    result_rx: tokio::sync::oneshot::Receiver<Result<R, StreamError>>,
    finished: bool,
}

#[derive(Debug)]
enum StreamEvent<T> {
    Event(T),
    Done,
}

#[derive(Debug, Clone)]
pub struct StreamError(pub String);

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamError: {}", self.0)
    }
}

impl<T, R> EventStream<T, R> {
    pub fn new() -> (EventStreamProducer<T, R>, Self) {
        let (event_tx, event_rx) = tokio::sync::mpsc::channel(64);
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();

        let producer = EventStreamProducer {
            event_tx,
            result_tx: Arc::new(Mutex::new(Some(result_tx))),
            finished: Arc::new(Mutex::new(false)),
        };

        let consumer = Self {
            receiver: event_rx,
            result_rx,
            finished: false,
        };

        (producer, consumer)
    }

    pub async fn result(self) -> Result<R, StreamError> {
        match self.result_rx.await {
            Ok(result) => result,
            Err(_) => Err(StreamError("Result channel closed".into())),
        }
    }
}

impl<T, R> Stream for EventStream<T, R> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }

        match Pin::new(&mut self.receiver).poll_recv(cx) {
            Poll::Ready(Some(StreamEvent::Event(event))) => Poll::Ready(Some(event)),
            Poll::Ready(Some(StreamEvent::Done)) => {
                self.finished = true;
                Poll::Ready(None)
            }
            Poll::Ready(None) => {
                self.finished = true;
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Clone)]
pub struct EventStreamProducer<T, R> {
    event_tx: tokio::sync::mpsc::Sender<StreamEvent<T>>,
    result_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<Result<R, StreamError>>>>>,
    finished: Arc<Mutex<bool>>,
}

impl<T, R> EventStreamProducer<T, R> {
    pub fn push(&self, event: T) {
        let _ = self.event_tx.try_send(StreamEvent::Event(event));
    }

    pub fn end(self, result: Result<R, StreamError>) {
        if let Some(tx) = self.result_tx.lock().take() {
            *self.finished.lock() = true;
            let _ = self.event_tx.try_send(StreamEvent::Done);
            let _ = tx.send(result);
        }
    }

    pub fn is_finished(&self) -> bool {
        *self.finished.lock()
    }
}
