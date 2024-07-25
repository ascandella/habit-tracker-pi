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
    foreground_color: Color,
    background_color: Color,
}

impl Display {
    pub fn new(gpio_chip: impl AsRef<std::path::Path>) -> Self {
        let busy = InputPin::new(&gpio_chip, 24).expect("busy pin");
        let dc = OutputPin::new(&gpio_chip, 25, PinState::Low).expect("DC pin");
        let rst = OutputPin::new(&gpio_chip, 17, PinState::Low).expect("RST pin");

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
        // TODO: Make a configuration option
        display.set_rotation(DisplayRotation::Rotate90);
        // TODO: Make a configuration option
        let foreground_color = Color::Black;
        let background_color = Color::White;

        display.clear(background_color).expect("clear screen");

        Self {
            display,
            spi,
            delay,
            device: epd2in7,
            foreground_color,
            background_color,
        }
    }

    pub fn height(&self) -> u32 {
        self.device.height()
    }

    #[allow(dead_code)]
    pub fn width(&self) -> u32 {
        self.device.width()
    }

    pub fn text(
        &mut self,
        text: &str,
        x: u32,
        y: u32,
        font: &embedded_graphics::mono_font::MonoFont<'_>,
    ) {
        let x = x.try_into().expect("x out of bounds");
        let y = y.try_into().expect("y out of bounds");
        let style = MonoTextStyleBuilder::new()
            .font(font)
            .text_color(self.foreground_color)
            .background_color(self.background_color)
            .build();

        let text_style = TextStyleBuilder::new()
            .baseline(Baseline::Top)
            .alignment(Alignment::Left)
            .build();

        // Infallible
        let _ = Text::with_text_style(text, Point::new(x, y), style, text_style)
            .draw(&mut self.display);
    }

    fn update(&mut self) {
        self.device
            .update_and_display_frame(&mut self.spi, self.display.buffer(), &mut self.delay)
            .expect("Update and display frame error");
    }

    pub fn clear(&mut self) {
        self.display
            .clear(self.background_color)
            .expect("Infallible clear");
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

fn day_text(count: i64) -> &'static str {
    match count {
        1 => "day",
        _ => "days",
    }
}

impl ui::TrackerDisplay for Display {
    fn display_streak(
        &mut self,
        timezone: &impl chrono::TimeZone,
        current: &db::StreakData,
        previous: &db::StreakData,
    ) {
        self.wake_up();
        self.clear();

        let current_count = match current {
            db::StreakData::NoData => 0,
            db::StreakData::Streak(streak) => streak.days(timezone),
        };
        let current_text = format!("{} {}", current_count, day_text(current_count));

        debug!(current_text, "Displaying current streak");
        self.text(
            &current_text,
            10,
            self.height() / 4,
            &profont::PROFONT_24_POINT,
        );

        let (previous_text, previous_start) = match previous {
            db::StreakData::NoData => ("No previous streak".into(), None),
            db::StreakData::Streak(streak) => {
                let text = format!(
                    "Previous: {} {}",
                    streak.count(),
                    day_text(streak.days(timezone)),
                );
                let date = Some(
                    streak
                        .end()
                        .with_timezone(timezone)
                        .fixed_offset()
                        .format("Ended %A, %B %d")
                        .to_string(),
                );
                (text, date)
            }
        };

        debug!(previous_text, previous_start, "Displaying previous streak");
        self.text(
            &previous_text,
            10,
            (self.width() * 3) / 4,
            &profont::PROFONT_12_POINT,
        );

        if let Some(previous_date) = previous_start {
            self.text(
                &previous_date,
                10,
                // Attempt to put on the bottom
                self.width() - 22,
                &profont::PROFONT_12_POINT,
            );
        }

        self.update();

        self.sleep().expect("sleep screen");
    }

    fn clear_and_shutdown(&mut self) {
        info!("Waking up for shutdown");
        self.wake_up();
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
}
