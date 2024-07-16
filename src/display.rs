use embedded_graphics::{
    mono_font::MonoTextStyleBuilder,
    prelude::*,
    primitives::{Line, PrimitiveStyleBuilder},
    text::{Baseline, Text, TextStyleBuilder},
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
    gpio_cdev::{Chip, LineHandle, LineRequestFlags},
    spidev::{self, SpidevOptions},
    Delay, SPIError, SpidevDevice,
};

pub(crate) struct Display {
    device: Epd2in7<SpidevDevice, InputPin, OutputPin, OutputPin, Delay>,
    display: Display2in7,
    spi: SpidevDevice,
    delay: Delay,
}

// Raspberry pi default GPIO cdev
const CHIP: &str = "/dev/gpiochip0";

impl Display {
    pub fn new() -> Self {
        let busy = InputPin::new(CHIP, 24).expect("busy pin");

        let dc = OutputPin::new(CHIP, 25, PinState::Low).expect("DC pin");

        let rst = OutputPin::new(CHIP, 17, PinState::Low).expect("RST pin");

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
        // display.clear();

        Self {
            display,
            spi,
            delay,
            device: epd2in7,
        }
    }

    pub fn text(&mut self, text: &str, x: i32, y: i32) {
        let style = MonoTextStyleBuilder::new()
            .font(&embedded_graphics::mono_font::ascii::FONT_6X10)
            .text_color(Color::Black)
            .background_color(Color::White)
            .build();

        let text_style = TextStyleBuilder::new().baseline(Baseline::Top).build();

        // Infallible
        let resp = Text::with_text_style(text, Point::new(x, y), style, text_style)
            .draw(&mut self.display);
        println!("Text response {:?}", resp);

        let style = PrimitiveStyleBuilder::new()
            .stroke_color(Color::Black)
            .stroke_width(1)
            .build();

        let _ = Line::new(Point::new(64, 64), Point::new(80, 80))
            .into_styled(style)
            .draw(&mut self.display);

        self.device
            .update_frame(&mut self.spi, self.display.buffer(), &mut self.delay)
            .expect("Update frame error");
        self.device
            .display_frame(&mut self.spi, &mut self.delay)
            .expect("Display frame error");
    }

    pub fn clear(&mut self) {
        self.display.clear(Color::White).expect("Infallible clear");
    }

    pub fn wake_up(&mut self) {
        self.device
            .wake_up(&mut self.spi, &mut self.delay)
            .expect("Unable to wake")
    }

    pub fn sleep(&mut self) -> Result<(), SPIError> {
        self.device.sleep(&mut self.spi, &mut self.delay)
    }
}
