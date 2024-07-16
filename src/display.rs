use embedded_graphics::{
    mono_font::MonoTextStyleBuilder,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle},
    text::{Baseline, Text, TextStyleBuilder},
};
use epd_waveshare::{
    color::*,
    epd2in7_v2::{Display2in7, Epd2in7},
    graphics::DisplayRotation,
    prelude::*,
};
use linux_embedded_hal::{
    spidev::{self, SpidevOptions},
    sysfs_gpio::Direction,
    Delay, SPIError, SpidevDevice, SysfsPin,
};

pub(crate) struct Display {
    device: Epd2in7<SpidevDevice, SysfsPin, SysfsPin, SysfsPin, Delay>,
    display: Display2in7,
    spi: SpidevDevice,
    delay: Delay,
}

impl Display {
    pub fn new() -> Self {
        let mut spi = SpidevDevice::open("/dev/spidev0.0").expect("spidev directory");
        let options = SpidevOptions::new()
            .bits_per_word(8)
            .max_speed_hz(4_000_000)
            .mode(spidev::SpiModeFlags::SPI_MODE_0)
            .build();
        spi.configure(&options).expect("spi configuration");

        // Configure Digital I/O Pin to be used as Chip Select for SPI
        let cs = SysfsPin::new(8);
        cs.export().expect("cs export");
        while !cs.is_exported() {}
        cs.set_direction(Direction::Out).expect("CS Direction");
        cs.set_value(1).expect("CS Value set to 1");

        let busy = SysfsPin::new(24); // GPIO 24, board J-18
        busy.export().expect("busy export");
        while !busy.is_exported() {}
        busy.set_direction(Direction::In).expect("busy Direction");

        let dc = SysfsPin::new(25); // GPIO 25, board J-22
        dc.export().expect("dc export");
        while !dc.is_exported() {}
        dc.set_direction(Direction::Out).expect("dc Direction");
        dc.set_value(1).expect("dc Value set to 1");

        let rst = SysfsPin::new(17); // GPIO 17, board J-11
        rst.export().expect("rst export");
        while !rst.is_exported() {}
        rst.set_direction(Direction::Out).expect("rst Direction");
        rst.set_value(1).expect("rst Value set to 1");

        let mut delay = Delay {};

        let epd2in7 =
            Epd2in7::new(&mut spi, busy, dc, rst, &mut delay, None).expect("eink initalize error");
        let mut display = Display2in7::default();
        display.set_rotation(DisplayRotation::Rotate0);

        Self {
            display,
            spi,
            delay,
            device: epd2in7,
        }
    }

    pub fn sleep(&mut self) -> Result<(), SPIError> {
        self.device.sleep(&mut self.spi, &mut self.delay)
    }
}
