use std::sync::Arc;

use tokio::{
    signal::unix::{SignalKind, signal},
    sync::watch,
};
use tokio_util::sync::CancellationToken;

use crate::{
    audio::AudioAdapter,
    config::ServiceConfig,
    serial,
    slider::{SliderFrame, SliderSmoother},
};

pub async fn run(config: ServiceConfig, adapter: Arc<dyn AudioAdapter>) -> anyhow::Result<()> {
    let shutdown = CancellationToken::new();
    let (tx, rx) = watch::channel(SliderFrame::new());

    let serial_task = tokio::spawn(serial::run(
        config.com_port.clone(),
        config.baud_rate,
        config.invert_sliders,
        tx,
        shutdown.clone(),
    ));

    let processing_task = tokio::spawn(process_frames(config, adapter, rx, shutdown.clone()));

    wait_for_shutdown_signal().await;
    log::info!("shutting down...");
    shutdown.cancel();

    let _ = tokio::join!(serial_task, processing_task);
    Ok(())
}

async fn wait_for_shutdown_signal() {
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = sigterm.recv() => {}
    }
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
            _ = shutdown.cancelled() => break,
            changed = rx.changed() => {
                if changed.is_err() {
                    log::error!("serial channel closed unexpectedly");
                    break;
                }
            }
        }

        let frame = rx.borrow_and_update().clone();
        for (index, value) in smoother.update(&frame) {
            let Ok(index) = u8::try_from(index) else {
                log::warn!("slider index {index} out of range, skipping");
                continue;
            };

            let Some(targets) = config.slider_mapping.get(&index) else {
                continue;
            };

            for target in targets {
                if let Err(err) = adapter.set_volume(target, value).await {
                    log::warn!("{err:#}");
                }
            }
        }
    }

    log::debug!("processing task shut down");
}
