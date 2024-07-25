use crossbeam_channel::{bounded, select};
use gpiocdev::line::EdgeDetection;
use std::error::Error;
use std::time::Duration;
use tracing::level_filters::LevelFilter;
use tracing::{error, info, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

mod display;
use display::Display;

// TODO: Take as command-line argument or otherwise make configurable
const GPIO_BUTTON: u32 = 26;
// Raspberry pi default GPIO cdev
const GPIO_CHIP: &str = "/dev/gpiochip0";

fn init_logging() {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(env_filter)
        .init();
}

fn main() -> Result<(), Box<dyn Error>> {
    init_logging();
    let (button_tx, button_rx) = bounded(1);

    info!(pin = GPIO_BUTTON, "Initializing GPIO for button");
    let pin_req = gpiocdev::Request::builder()
        .on_chip(GPIO_CHIP)
        .with_consumer("workout tracker")
        .with_line(GPIO_BUTTON)
        .with_bias(gpiocdev::line::Bias::PullUp) // The other end of the button is connected
        // to ground, pull up to detect easier
        .with_edge_detection(EdgeDetection::FallingEdge)
        .request()?;

    let mut button = ui::DebouncedButton::new(button_tx, Duration::from_millis(500));

    std::thread::spawn(move || {
        for _event in pin_req.edge_events() {
            button.pressed();
        }
    });

    let (exit_tx, exit_rx) = bounded(1);

    ctrlc::set_handler(move || exit_tx.send(()).expect("Could not send signal on channel"))?;

    info!("Initializing e-ink display");
    let eink = Display::new(GPIO_CHIP);

    info!("Opening database");
    // TODO: Make file path a parameter
    let db = db::open_file("tracker.db")?;
    let mut interface = ui::HabitInterface::new(eink, db, &chrono_tz::US::Pacific);

    info!("Refreshing initial stats");
    interface.refresh_stats().expect("refresh stats");

    // TODO: Add screen clear / sleep + wake up outside of waking hours to avoid burn-in
    let mut running = true;
    while running {
        select! {
            recv(button_rx) -> _ => {
                if let Err(err) = interface.button_pressed() {
                    error!(%err, "Error recording event");
                }
            }
            recv(exit_rx) -> _ => {
                warn!("Received control-c. Exiting...");
                running = false;
            }
        }
    }

    if let Err(err) = interface.shutdown() {
        error!(%err, "Error shutting down interface");
    }
    info!("Shutdown complete, exiting");

    Ok(())
}
