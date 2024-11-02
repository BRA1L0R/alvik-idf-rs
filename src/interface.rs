use esp_idf_svc::{
    hal::{
        gpio::{AnyIOPin, IOPin, InputOutput, InputPin, OutputPin, PinDriver},
        peripheral::Peripheral,
        uart::{Uart, UartConfig, UartDriver},
    },
    sys::EspError,
};

pub struct AlvikInterface {
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
