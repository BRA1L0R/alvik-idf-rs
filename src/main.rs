pub mod command;
pub mod dispatcher;
pub mod interface;
pub mod serial;

use std::{
    ops::ControlFlow,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use command::Message;
use dispatcher::Handler;
use esp_idf_svc::{hal::prelude::Peripherals, mqtt::client::Event, sys::EspError};
use futures::executor::block_on;
use interface::AlvikInterface;
use serial::{
    channel::{AlvikChannel, Rx},
    AlvikSerial,
};
use thiserror::Error;

#[derive(Error, Debug)]
enum AlvikError {
    #[error("alvik is turned off")]
    Offline,
    #[error("esp error: {0}")]
    Esp(#[from] EspError),
}

struct AlvikBuilder<H: Handler<Message>> {
    interface: AlvikInterface,
    // subscribers: Vec<DynSubscriber>,
    handler: H,
}

impl AlvikBuilder<()> {
    fn new(interface: AlvikInterface) -> Self {
        AlvikBuilder {
            interface,
            handler: (),
        }
    }
}

impl<H: Handler<Message> + Send + 'static> AlvikBuilder<H> {
    pub fn subscribe<S>(self, handler: S) -> AlvikBuilder<impl Handler<Message>>
    where
        S: Handler<Message> + Send + 'static,
    {
        use dispatcher::HandlerExt;
        let new_handler = self.handler.chain_to(handler);

        AlvikBuilder {
            interface: self.interface,
            handler: new_handler,
        }
    }

    fn build(self) -> Result<AlvikDriver, AlvikError> {
        // AlvikDriver::begin(self.interface, self.subscribers)
        AlvikDriver::begin(self.interface, self.handler)
    }
}

struct AlvikDriver {
    // pub nrst: PinDriver<'static, AnyIOPin, InputOutput>,
    // pub check: PinDriver<'static, AnyIOPin, InputOutput>,
    serial: AlvikSerial,
    // subscribers: Vec<Box<dyn Subscriber>>,
}

async fn dispatcher(rx: AlvikChannel<Rx>, subscribers: impl Handler<Message>) {
    loop {
        let message = rx.recv().await;
        subscribers.handle_event(message);
    }
}

impl AlvikDriver {
    fn begin(
        AlvikInterface {
            mut nrst,
            mut check,
            uart,
        }: AlvikInterface,
        subscribers: impl Handler<Message> + Send + 'static,
    ) -> Result<Self, AlvikError> {
        use esp_idf_svc::hal::gpio::Pull;
        check.set_pull(Pull::Down).unwrap();

        if check.is_low() {
            log::error!("Alvik is offline!");
            return Err(AlvikError::Offline);
        };

        // reset alvik
        nrst.set_low()?;
        std::thread::sleep(Duration::from_millis(200));
        nrst.set_high()?;
        std::thread::sleep(Duration::from_millis(200));

        let mut serial = AlvikSerial::spawn(uart);
        let receiver = serial.take_receiver().unwrap();

        std::thread::spawn(move || block_on(dispatcher(receiver, subscribers)));

        Ok(Self { serial })
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

    #[derive(Default)]
    struct IMU {
        x: AtomicU32,
        y: AtomicU32,
        z: AtomicU32,
    }

    impl IMU {
        fn get(&self) -> (f32, f32, f32) {
            let x = self.x.load(Ordering::Relaxed).to_le_bytes();
            let y = self.y.load(Ordering::Relaxed).to_le_bytes();
            let z = self.z.load(Ordering::Relaxed).to_le_bytes();

            (
                f32::from_le_bytes(x),
                f32::from_le_bytes(y),
                f32::from_le_bytes(z),
            )
        }
    }

    impl Handler<Message> for IMU {
        fn handle_event(&self, event: Message) -> ControlFlow<(), Message> {
            let Message::ImuPosition { roll, pitch, yaw } = event else {
                return ControlFlow::Continue(event);
            };

            self.x
                .store(u32::from_le_bytes(roll.to_le_bytes()), Ordering::Relaxed);
            self.y
                .store(u32::from_le_bytes(pitch.to_le_bytes()), Ordering::Relaxed);
            self.z
                .store(u32::from_le_bytes(yaw.to_le_bytes()), Ordering::Relaxed);

            ControlFlow::Break(())
        }
    }

    let imu = Arc::new(IMU::default());

    AlvikBuilder::new(interface)
        .subscribe(imu.clone())
        .build()
        .unwrap();

    loop {
        std::thread::sleep(Duration::from_secs(1));
        let data = imu.get();

        println!("{data:?}");
    }

    // log::info!("Driver started!");
}
