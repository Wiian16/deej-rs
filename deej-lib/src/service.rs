use std::sync::Arc;

use futures::future::join_all;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::{
    audio::AudioAdapter,
    config::ServiceConfig,
    serial,
    slider::{SliderFrame, SliderSmoother},
};

/// Spawn and run all main deej tasks.
pub async fn run(
    config: ServiceConfig,
    adapter: Arc<dyn AudioAdapter>,
    shutdown: CancellationToken,
) {
    let (tx, rx) = watch::channel(SliderFrame::new());

    let serial_task = tokio::spawn(serial::run(
        config.com_port.clone(),
        config.baud_rate,
        config.invert_sliders,
        tx,
        shutdown.clone(),
    ));

    let processing_task = tokio::spawn(process_frames(config, adapter, rx, shutdown.clone()));

    shutdown.cancelled().await;

    let _ = tokio::join!(serial_task, processing_task);
}

async fn process_frames(
    config: ServiceConfig,
    adapter: Arc<dyn AudioAdapter>,
    mut rx: watch::Receiver<SliderFrame>,
    shutdown: CancellationToken,
) {
    let mut smoother = SliderSmoother::new(config.noise_reduction);

    loop {
        tokio::select! {
            biased;
            () = shutdown.cancelled() => break,
            changed = rx.changed() => {
                if changed.is_err() {
                    if shutdown.is_cancelled() {
                        log::error!("serial channel closed unexpectedly");
                    }
                    else {
                        log::debug!("serial channel closed during shutdown");
                    }
                    break;
                }
            }
        }

        let frame = rx.borrow_and_update().clone();
        let mut futures = Vec::new();
        for (index, value) in smoother.update(&frame) {
            let Ok(index) = u8::try_from(index) else {
                log::warn!("slider index {index} out of range, skipping");
                continue;
            };

            let Some(targets) = config.slider_mapping.get(&index) else {
                continue;
            };

            for target in targets {
                futures.push(adapter.set_volume(target, value));
            }
        }

        for result in join_all(futures).await {
            if let Err(err) = result {
                log::warn!("{err:#}");
            }
        }
    }

    log::debug!("processing task shut down");
}
