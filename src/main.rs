pub mod command;
pub mod interface;
pub mod serial;

use std::time::Duration;

use esp_idf_svc::{
    hal::{
        gpio::{AnyIOPin, InputOutput, PinDriver},
        prelude::Peripherals,
    },
    sys::EspError,
};
use interface::AlvikInterface;
use serial::{AlvikChannel, AlvikSerial, Rx};
use thiserror::Error;

#[derive(Error, Debug)]
enum AlvikError {
    #[error("alvik is turned off")]
    Offline,
    #[error("esp error: {0}")]
    Esp(#[from] EspError),
}

struct AlvikDriver {
    pub nrst: PinDriver<'static, AnyIOPin, InputOutput>,
    pub check: PinDriver<'static, AnyIOPin, InputOutput>,
    serial: AlvikSerial,
}

impl AlvikDriver {
    fn begin(
        AlvikInterface {
            mut nrst,
            mut check,
            uart,
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

        let mut serial = AlvikSerial::spawn(uart);
        let receiver: AlvikChannel<Rx> = serial.take_receiver().unwrap();

        Ok(Self {
            nrst,
            check,
            serial,
        })
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
    let driver = AlvikDriver::begin(interface).unwrap();

    log::info!("Driver started!");
}
