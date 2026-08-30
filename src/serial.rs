use std::time::Duration;

use futures::StreamExt;
use tokio::sync::watch;
use tokio_serial::SerialPortBuilderExt;
use tokio_util::{
    codec::{FramedRead, LinesCodec},
    sync::CancellationToken,
};

use crate::slider::{SliderFrame, parse_line};

const MAX_ADC_VALUE: u16 = 1023;
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

pub async fn run(
    port_name: String,
    baud_rate: u32,
    invert: bool,
    tx: watch::Sender<SliderFrame>,
    shutdown: CancellationToken,
) {
    while !shutdown.is_cancelled() {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            result = connect_and_stream(&port_name, baud_rate, invert, &tx, &shutdown) => {
                if let Err(err) = result {
                    log::warn!(
                        "serial connection to {port_name} lost: {err:#}. Reconnecting in {RECONNECT_DELAY:?}"
                    )
                }
            }
        }

        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = tokio::time::sleep(RECONNECT_DELAY) => {}
        }
    }
}

async fn connect_and_stream(
    port_name: &str,
    baud_rate: u32,
    invert: bool,
    tx: &watch::Sender<SliderFrame>,
    shutdown: &CancellationToken,
) -> anyhow::Result<()> {
    let port = tokio_serial::new(port_name, baud_rate).open_native_async()?;
    log::info!("connected to {port_name} @ {baud_rate} baud");

    let mut lines = FramedRead::new(port, LinesCodec::new());
    loop {
        let next = tokio::select! {
            _ = shutdown.cancelled() => return Ok(()),
            next = lines.next() => next,
        };

        match next {
            Some(Ok(line)) => {
                if let Some(frame) = parse_line(&line, MAX_ADC_VALUE, invert) {
                    let _ = tx.send(frame); // Error here means shutting down, safe to ignore. 
                } else {
                    log::trace!("ignoring unparsable serial line: {line:?}")
                }
            }
            Some(Err(err)) => anyhow::bail!(err),
            None => anyhow::bail!("serial port closed (EOF)"),
        }
    }
}
