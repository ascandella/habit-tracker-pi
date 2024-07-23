use embedded_graphics::{
    mono_font::MonoTextStyleBuilder,
    prelude::*,
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use embedded_hal::digital::PinState;
use epd_waveshare::{
    color::*,
    epd2in7_v2::{Display2in7, Epd2in7},
    graphics::DisplayRotation,
    prelude::*,
};
use gpiocdev_embedded_hal::{InputPin, OutputPin};
use linux_embedded_hal::{
    spidev::{self, SpidevOptions},
    Delay, SPIError, SpidevDevice,
};
use tracing::{debug, info};

pub(crate) struct Display {
    device: Epd2in7<SpidevDevice, InputPin, OutputPin, OutputPin, Delay>,
    display: Display2in7,
    spi: SpidevDevice,
    delay: Delay,
}

impl Display {
    pub fn new(gpio_chip: &str) -> Self {
        let busy = InputPin::new(gpio_chip, 24).expect("busy pin");

        let dc = OutputPin::new(gpio_chip, 25, PinState::Low).expect("DC pin");

        let rst = OutputPin::new(gpio_chip, 17, PinState::Low).expect("RST pin");

        let mut spi = SpidevDevice::open("/dev/spidev0.0").expect("spidev directory");
        let options = SpidevOptions::new()
            .bits_per_word(8)
            .max_speed_hz(4_000_000)
            .mode(spidev::SpiModeFlags::SPI_MODE_0)
            .build();
        spi.configure(&options).expect("spi configuration");

        let mut delay = Delay {};

        let epd2in7 =
            Epd2in7::new(&mut spi, busy, dc, rst, &mut delay, None).expect("eink initalize error");
        let mut display = Display2in7::default();
        display.set_rotation(DisplayRotation::Rotate90);
        display.clear(Color::White).expect("clear screen");

        Self {
            display,
            spi,
            delay,
            device: epd2in7,
        }
    }

    pub fn height(&self) -> u32 {
        self.device.height()
    }

    pub fn width(&self) -> u32 {
        self.device.width()
    }

    pub fn text(&mut self, text: &str, x: u32, y: u32) {
        let x = x.try_into().expect("x out of bounds");
        let y = y.try_into().expect("y out of bounds");
        let style = MonoTextStyleBuilder::new()
            .font(&profont::PROFONT_24_POINT)
            .text_color(Color::Black)
            .background_color(Color::White)
            .build();

        let text_style = TextStyleBuilder::new()
            .baseline(Baseline::Middle)
            .alignment(Alignment::Center)
            .build();

        // Infallible
        let _ = Text::with_text_style(text, Point::new(x, y), style, text_style)
            .draw(&mut self.display);

        self.device
            .update_and_display_frame(&mut self.spi, self.display.buffer(), &mut self.delay)
            .expect("Update and display frame error");
    }

    pub fn clear(&mut self) {
        self.display.clear(Color::White).expect("Infallible clear");
    }

    pub fn clear_and_shutdown(&mut self) {
        info!("Clearing screen for shutdown");
        self.clear();
        self.device
            .clear_frame(&mut self.spi, &mut self.delay)
            .expect("Unable to clear frame");
        self.device
            .display_frame(&mut self.spi, &mut self.delay)
            .expect("Unable to display cleared frame");
        self.sleep().expect("Unable to sleep");
    }

    pub fn wake_up(&mut self) {
        debug!("Waking screen up");
        self.device
            .wake_up(&mut self.spi, &mut self.delay)
            .expect("Unable to wake")
    }

    pub fn sleep(&mut self) -> Result<(), SPIError> {
        debug!("Putting screen to sleep");
        self.device.sleep(&mut self.spi, &mut self.delay)
    }
}
