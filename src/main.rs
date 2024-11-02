pub mod command;

use std::{convert::Infallible, sync::Arc, thread::JoinHandle, time::Duration};

use command::Message;
use embassy_sync::{
    blocking_mutex::raw::NoopRawMutex,
    channel::{Channel, Sender},
};
use esp_idf_svc::{
    hal::{
        gpio::{AnyIOPin, IOPin, InputOutput, InputPin, OutputPin, PinDriver},
        peripheral::Peripheral,
        prelude::Peripherals,
        task::{block_on, embassy_sync::EspRawMutex},
        uart::{AsyncUartDriver, AsyncUartRxDriver, Uart, UartConfig, UartDriver, UartRxDriver},
    },
    sys::EspError,
};
use futures::future::join;
use thiserror::Error;
use ucpack::{is_complete_message, UcPack};

#[derive(Error, Debug)]
enum AlvikError {
    #[error("alvik is turned off")]
    Offline,
    #[error("esp error: {0}")]
    Esp(#[from] EspError),
}

struct AlvikSerial {
    handle: JoinHandle<()>,
    send_channel: AlvikChannel,
    recv_channel: AlvikChannel,
}

type AlvikChannel = Arc<Channel<EspRawMutex, Message, 50>>;

async fn alvik_task(
    uart: UartDriver<'static>,
    send_channel: AlvikChannel,
    receive_channel: AlvikChannel,
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
            // log::info!("read {read} bytes");
            cursor += read;

            let mut partial = 0;
            while let Some(complete) = is_complete_message(&receive_buffer[partial..cursor]) {
                let message: Message = match PACK.deserialize_slice(complete) {
                    Ok(msg) => msg,
                    Err(err) => {
                        log::error!("error {err}");
                        log::error!("{complete:x?}");
                        panic!()
                    }
                };

                receive_channel.send(message).await;
                partial += complete.len();
            }

            receive_buffer.copy_within(partial.., 0);
            cursor -= partial;
        }
    };

    let send_task = async move {
        // while let a  = send_channel.receive().await {};
        loop {
            let message = send_channel.receive().await;
            let serialized = PACK
                .serialize_slice(&message, &mut send_buffer[..])
                .unwrap();

            uart_tx.write(&send_buffer[..serialized]).await.unwrap();
        }
    };

    // start the receive and send subroutines
    join(receive_task, send_task).await;

    panic!("left read loop");
}

impl AlvikSerial {
    pub fn spawn(uart: UartDriver<'static>) -> Self {
        let send_channel = Arc::new(Channel::<EspRawMutex, Message, 50>::new());
        let recv_channel = Arc::new(Channel::<EspRawMutex, Message, 50>::new());

        let handle = {
            let send_channel = send_channel.clone();
            let recv_channel = recv_channel.clone();
            std::thread::spawn(move || {
                let Err(err) = block_on(alvik_task(uart, send_channel, recv_channel));
                panic!("alvik receiver task returned with error: {err}");
            })
        };

        Self {
            handle,
            send_channel,
            recv_channel,
        }
    }
}

struct AlvikDriver {
    pub nrst: PinDriver<'static, AnyIOPin, InputOutput>,
    pub check: PinDriver<'static, AnyIOPin, InputOutput>,
    serial: AlvikSerial,
}

impl AlvikDriver {
    fn init(
        AlvikInterface {
            mut nrst,
            mut check,
            mut uart,
        }: AlvikInterface,
    ) -> Result<Self, AlvikError> {
        use esp_idf_svc::hal::gpio::Pull;
        check.set_pull(Pull::Down).unwrap();

        if check.is_low() {
            return Err(AlvikError::Offline);
        };

        // reset alvik
        nrst.set_low().unwrap();
        std::thread::sleep(Duration::from_millis(200));
        nrst.set_high().unwrap();
        std::thread::sleep(Duration::from_millis(200));

        let serial = AlvikSerial::spawn(uart);

        serial
            .send_channel
            .try_send(Message::SetLed { value: 0xFF })
            .unwrap();

        block_on(async move {
            let mut received = 0;
            loop {
                let msg = serial.recv_channel.receive().await;
                received += 1;

                if received % 20 == 0 {
                    log::info!("{msg:?}");
                }
            }
        });

        todo!();
    }
}

struct AlvikInterface {
    pub nrst: PinDriver<'static, AnyIOPin, InputOutput>,
    pub check: PinDriver<'static, AnyIOPin, InputOutput>,
    pub uart: UartDriver<'static>,
}

impl AlvikInterface {
    pub fn from_pins(
        nrst: impl IOPin,
        check: impl IOPin,
        uart: impl Peripheral<P = impl Uart> + 'static,
        tx: impl OutputPin,
        rx: impl InputPin,
    ) -> Result<Self, EspError> {
        let (nrst, check) = (
            PinDriver::input_output(nrst.downgrade())?,
            PinDriver::input_output(check.downgrade())?,
        );

        let config = UartConfig::new().baudrate(460800.into());
        let uart = UartDriver::new(
            uart,
            tx,
            rx,
            Option::<AnyIOPin>::None,
            Option::<AnyIOPin>::None,
            &config,
        )?;

        Ok(Self { nrst, check, uart })
    }
}

fn main() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("Starting the Rust alvik!");

    let peripherals = Peripherals::take().unwrap();

    let interface = AlvikInterface::from_pins(
        peripherals.pins.gpio6,
        peripherals.pins.gpio13,
        peripherals.uart1,
        peripherals.pins.gpio43,
        peripherals.pins.gpio44,
    )
    .unwrap();
    let driver = AlvikDriver::init(interface).unwrap();

    log::info!("Driver started!");
}
