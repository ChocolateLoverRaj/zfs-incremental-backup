use std::time::Duration;

use spinners::{Spinner, Spinners};

pub async fn sleep_with_spinner(duration: Duration) {
    let mut spinner = Spinner::with_timer(Spinners::Dots, format!("Sleeping for {duration:?}"));
    tokio::time::sleep(duration).await;
    spinner.stop_with_newline();
}
