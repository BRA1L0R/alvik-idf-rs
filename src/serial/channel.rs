use std::{marker::PhantomData, sync::Arc};

use embassy_sync::channel::{Channel, TrySendError};
use esp_idf_svc::hal::task::embassy_sync::EspRawMutex;

use crate::command::Message;

pub struct Rx;
pub struct Tx;

pub struct AlvikChannel<End, const N: usize = 50> {
    inner: Arc<Channel<EspRawMutex, Message, N>>,
    _end: PhantomData<End>,
}

impl AlvikChannel<()> {
    pub fn bound<const N: usize>() -> (AlvikChannel<Tx, N>, AlvikChannel<Rx, N>) {
        let channel = Arc::new(Channel::<EspRawMutex, Message, N>::new());

        let write: AlvikChannel<Tx, N> = AlvikChannel {
            inner: channel.clone(),
            _end: PhantomData::default(),
        };

        let read: AlvikChannel<Rx, N> = AlvikChannel {
            inner: channel,
            _end: PhantomData::default(),
        };

        (write, read)
    }
}

impl<const N: usize> AlvikChannel<Rx, N> {
    pub async fn recv(&self) -> Message {
        self.inner.receive().await
    }
}

impl<const N: usize> AlvikChannel<Tx, N> {
    pub fn try_send(&self, message: Message) -> Result<(), TrySendError<Message>> {
        self.inner.try_send(message)
    }

    pub async fn send(&self, message: Message) {
        self.inner.send(message).await
    }
}

impl<const N: usize> Clone for AlvikChannel<Tx, N> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _end: Default::default(),
        }
    }
}
