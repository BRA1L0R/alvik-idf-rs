pub mod channel;

use std::{convert::Infallible, thread::JoinHandle};

use channel::{AlvikChannel, Rx, Tx};
use esp_idf_svc::hal::{
    task::block_on,
    uart::{AsyncUartDriver, UartDriver},
};
use futures::future::join;
use ucpack::UcPack;

use crate::{command::Message, AlvikError};

pub struct AlvikSerial {
    _handle: JoinHandle<()>,
    recv_channel: Option<AlvikChannel<Rx>>,
    send_channel: AlvikChannel<Tx>,
}

async fn alvik_task(
    uart: UartDriver<'static>,
    send_rx: AlvikChannel<Rx>,
    recv_tx: AlvikChannel<Tx>,
) -> Result<Infallible, AlvikError> {
    let mut uart = AsyncUartDriver::wrap(uart)?;
    uart.driver().clear_rx()?;

    let (uart_tx, uart_rx) = uart.split();

    let mut receive_buffer = Box::new([0u8; 512]);
    let mut send_buffer = Box::new([0u8; 512]);

    const PACK: UcPack = UcPack::new(b'A', b'#');

    let receive_task = async move {
        let mut cursor = 0;
        while let Ok(read) = uart_rx.read(&mut receive_buffer[cursor..]).await {
            cursor += read;

            let mut partial = 0;
            'messages: while let Some(complete) =
                ucpack::is_complete_message(&receive_buffer[partial..cursor])
            {
                partial += complete.len();

                let message = PACK.deserialize_slice(complete);
                let message: Message = match message {
                    Ok(msg) => msg,
                    Err(err) => {
                        log::error!("Error while decoding a packet: {err}");
                        continue 'messages; // show must go on
                    }
                };

                // once a message has been read send it through
                // the fifo
                if let Err(_) = recv_tx.try_send(message) {
                    log::warn!("Failed to push update in the receive channel. Is the channel full?")
                };
            }

            // partials that have been not serialized must go back
            // to the start of the buffer
            receive_buffer.copy_within(partial.., 0);
            cursor -= partial;
        }
    };

    let send_task = async move {
        // while let a  = send_channel.receive().await {};
        loop {
            let message = send_rx.recv().await;
            let serialized = PACK
                .serialize_slice(&message, &mut send_buffer[..])
                .unwrap();

            uart_tx.write(&send_buffer[..serialized]).await.unwrap();
        }
    };

    // start the receive and send subroutines
    join(receive_task, send_task).await;
    unreachable!(); // todo: change this
}

impl AlvikSerial {
    pub fn spawn(uart: UartDriver<'static>) -> Self {
        let (send_tx, send_rx) = AlvikChannel::bound::<50>();
        let (recv_tx, recv_rx) = AlvikChannel::bound::<50>();

        let handle = {
            std::thread::spawn(move || {
                let Err(err) = block_on(alvik_task(uart, send_rx, recv_tx));
                panic!("alvik receiver task returned with error: {err}");
            })
        };

        Self {
            _handle: handle,
            send_channel: send_tx,
            recv_channel: Some(recv_rx),
        }
    }

    /// takes the message receiver of which only
    /// one concurrent instance can exist at any given time
    pub fn take_receiver(&mut self) -> Option<AlvikChannel<Rx>> {
        self.recv_channel.take()
    }
}
