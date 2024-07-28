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

fn next_midnight(tz: &impl chrono::TimeZone) -> Option<chrono::DateTime<chrono::Utc>> {
    let now = chrono::Utc::now().with_timezone(tz);
    let midnight = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap();
    (now + chrono::Duration::days(1))
        .with_time(midnight)
        .single()
        .map(|dt| dt.to_utc())
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
    // TODO: Make configurable
    let timezone = chrono_tz::US::Pacific;
    let mut interface = ui::HabitInterface::new(eink, db.clone(), timezone);

    info!("Refreshing initial stats");
    interface.refresh_stats().expect("refresh stats");

    // Go to sleep at midnight
    let next_sleep = next_midnight(&timezone).expect("next midnight");

    let (wake_tx, wake_rx) = bounded(1);
    let (sleep_tx, sleep_rx) = bounded(1);

    std::thread::spawn(move || {
        let time_til_midnight = (next_sleep - chrono::Utc::now())
            .to_std()
            .expect("duration until midnight");
        std::thread::sleep(time_til_midnight);

        sleep_tx.send(()).expect("send to sleep channel");

        let one_day = chrono::Duration::days(1).to_std().expect("one day");
        let sleep_ticker = crossbeam_channel::tick(one_day);

        // Wake up at 5am
        let five_hours = std::time::Duration::from_secs(60 * 60 * 5);
        std::thread::sleep(five_hours);

        wake_tx.send(()).expect("send to wake channel");
        let wake_ticker = crossbeam_channel::tick(one_day);

        loop {
            select! {
                recv(sleep_ticker) -> _ => {
                    sleep_tx.send(()).expect("send to sleep channel");
                }
                recv(wake_ticker) -> _ => {
                    wake_tx.send(()).expect("send to wake channel");
                }
            }
        }
    });

    let tokio_rt = tokio::runtime::Runtime::new()?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(None);

    tokio_rt.spawn(async move {
        loop {
            select! {
                recv(sleep_rx) -> _ => {
                    info!("Received sleep signal");
                    interface.sleep();
                }
                recv(wake_rx) -> _ => {
                    info!("Received wakeup signal");
                    if let Err(err) = interface.refresh_stats() {
                        error!(%err, "Error refreshing stats for wakeup");
                    }
                }
                recv(button_rx) -> _ => {
                    if let Err(err) = interface.button_pressed() {
                        error!(%err, "Error recording event");
                    }
                }
                recv(exit_rx) -> _ => {
                    warn!("Received control-c. Exiting...");
                    shutdown_tx.send(Some(())).expect("send shutdown signal");

                    if let Err(err) = interface.shutdown() {
                        error!(%err, "Error shutting down interface");
                    }

                    break;
                }
            }
        }
    });

    tokio_rt.block_on(async {
        // TODO: Make configurable
        let port = 4124;
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
            .await
            .unwrap();
        let app = axum::Router::new().route("/", axum::routing::get(|| async { "Hello, World!" }));
        info!(port, "Web server listening");
        if let Err(err) = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal(shutdown_rx))
            .await
        {
            error!(%err, "Error running server");
        }
    });

    if let Err(err) = db.close() {
        error!(%err, "Error closing DB");
    }
    info!("Shutdown complete, exiting");

    Ok(())
}

async fn shutdown_signal(mut rx: tokio::sync::watch::Receiver<Option<()>>) {
    loop {
        if rx.changed().await.is_ok() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Timelike;

    use super::*;

    #[test]
    fn test_next_midnight() {
        let tz = chrono_tz::US::Pacific;
        let now = chrono::Utc::now();
        let midnight = next_midnight(&tz).unwrap();
        assert!(midnight > now);
        let midnight_local = midnight.with_timezone(&tz);
        assert_eq!(midnight_local.hour(), 0);
        assert_eq!(midnight_local.minute(), 0);
        assert_eq!(midnight_local.second(), 0);

        let delta = midnight - now;
        assert_eq!(delta.num_days(), 0);
    }
}
