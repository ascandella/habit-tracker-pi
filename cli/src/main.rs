use crossbeam_channel::{bounded, select};
use gpiocdev::line::EdgeDetection;
use std::error::Error;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

mod display;
use display::Display;

const GPIO_BUTTON: u32 = 26;
// Raspberry pi default GPIO cdev
const GPIO_CHIP: &str = "/dev/gpiochip0";

fn init_logging() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();
}

fn main() -> Result<(), Box<dyn Error>> {
    init_logging();
    let (button_tx, button_rx) = bounded(1);

    info!("Initializing GPIO");
    let pin_req = gpiocdev::Request::builder()
        .on_chip(GPIO_CHIP)
        .with_consumer("workout tracker")
        .with_line(GPIO_BUTTON)
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

    let eink = Display::new(GPIO_CHIP);

    let db = db::open_file("tracker.db")?;
    let mut interface = ui::HabitInterface::new(eink, db, &chrono_tz::US::Pacific);

    interface.refresh_stats().expect("refresh stats");

    let mut running = true;
    while running {
        select! {
            recv(button_rx) -> _ => {
                interface.button_pressed().expect("unable to handle button press");
            }
            recv(exit_rx) -> _ => {
                println!("Received control-c. Exiting...");
                running = false;
            }
        }
    }

    interface.shutdown().expect("shutdown interface");

    Ok(())
}
