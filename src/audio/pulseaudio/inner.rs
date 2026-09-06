use std::{
    sync::{
        Arc,
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
};

use libpulse_binding::{
    context::{Context, FlagSet, State},
    mainloop::threaded::Mainloop,
};
use tokio::sync::oneshot;

use crate::audio::pulseaudio::error::PulseError;

/// A handle for communicating into the background worker thread that owns the `libpulse` mainloop and context.
///
/// Send a [`Command::Run`] to run a function on the mainloop with it's context.
pub(crate) struct PulseInner {
    /// Option is used here internally so we can use `take` to drop the sender held inside, signalling to the thread
    /// that it should shut down.
    sender: Option<Sender<Command>>,
    thread_handle: Option<JoinHandle<()>>,
}

impl PulseInner {
    pub async fn new(name: String) -> Result<Arc<Self>, PulseError> {
        let (cmd_tx, thread_handle, ready) = spawn(name)?;

        ready.await.map_err(|_| PulseError::Disconnected)??;

        Ok(Arc::new(Self {
            sender: Some(cmd_tx),
            thread_handle: Some(thread_handle),
        }))
    }

    pub fn send(&self, command: Command) {
        if let Some(ref sender) = self.sender {
            let _ = sender.send(command);
        }
    }
}

impl Drop for PulseInner {
    fn drop(&mut self) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(Command::Shutdown);
        }

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }

        log::debug!("pulseaudio worker thread closed")
    }
}

pub enum Command {
    Run(Box<dyn FnOnce(&mut Context) + Send + 'static>),
    Shutdown,
    StateChanged,
}

fn spawn(
    app_name: String,
) -> Result<
    (
        mpsc::Sender<Command>,
        JoinHandle<()>,
        oneshot::Receiver<Result<(), PulseError>>,
    ),
    PulseError,
> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();
    let (ready_tx, ready_rx) = oneshot::channel();
    let self_tx = cmd_tx.clone();

    let thread_handle = thread::Builder::new()
        .name("pulseaudio-wrapper".to_string())
        .spawn(move || thread_main(app_name, self_tx, cmd_rx, ready_tx))?;

    Ok((cmd_tx, thread_handle, ready_rx))
}

fn thread_main(
    app_name: String,
    self_tx: mpsc::Sender<Command>,
    cmd_rx: mpsc::Receiver<Command>,
    ready_tx: oneshot::Sender<Result<(), PulseError>>,
) {
    let mut mainloop = match Mainloop::new() {
        Some(m) => m,
        None => {
            let _ = ready_tx.send(Err(PulseError::MainLoopCreation));
            return;
        }
    };

    let mut context = match Context::new(&mainloop, &app_name) {
        Some(c) => c,
        None => {
            let _ = ready_tx.send(Err(PulseError::ContextCreation));
            return;
        }
    };

    // State callback runs on PulseAudio's internal thread and can't reach back into this thread's `Context`, so we make
    // ping us, then we check get state ourselves on this thread. This also gives us disconnect detection.
    {
        let cb_tx = self_tx.clone();
        context.set_state_callback(Some(Box::new(move || {
            let _ = cb_tx.send(Command::StateChanged);
        })));
    }

    if let Err(e) = mainloop.start() {
        let _ = ready_tx.send(Err(PulseError::Pulse(e)));
        return;
    }

    {
        mainloop.lock();
        let connected = context.connect(None, FlagSet::NOFLAGS, None);
        mainloop.unlock();

        if let Err(e) = connected {
            shutdown(&mut mainloop, &mut context);
            let _ = ready_tx.send(Err(PulseError::Pulse(e)));
            return;
        }
    }

    // Wait for the connection to be ready
    let mut ready_tx = Some(ready_tx);
    loop {
        match cmd_rx.recv() {
            Ok(Command::StateChanged) => {
                mainloop.lock();
                let state = context.get_state();
                mainloop.unlock();

                match state {
                    State::Ready => {
                        if let Some(tx) = ready_tx.take() {
                            let _ = tx.send(Ok(()));
                        }

                        break;
                    }
                    State::Failed | State::Terminated => {
                        shutdown(&mut mainloop, &mut context);

                        if let Some(tx) = ready_tx.take() {
                            let _ = tx.send(Err(PulseError::ConnectionFailed));
                        }
                        return;
                    }
                    _ => {}
                }
            }
            Ok(Command::Run(_)) => {
                // Nobody should hold a `PulseWrapper` yet, so this can't happen.
            }
            Ok(Command::Shutdown) => {
                shutdown(&mut mainloop, &mut context);

                if let Some(tx) = ready_tx.take() {
                    let _ = tx.send(Err(PulseError::Disconnected));
                }
                return;
            }
            Err(_) => {
                // All senders were dropped, which can't happen this early since we hold one. Handle it anyways.
                shutdown(&mut mainloop, &mut context);

                if let Some(tx) = ready_tx.take() {
                    let _ = tx.send(Err(PulseError::Disconnected));
                }
                return;
            }
        }
    }

    // Steady state: run requests as they come in until told to stop or the connection dies
    for cmd in cmd_rx.iter() {
        match cmd {
            Command::Run(f) => {
                mainloop.lock();
                f(&mut context);
                mainloop.unlock();
            }
            Command::StateChanged => {
                mainloop.lock();
                let state = context.get_state();
                mainloop.unlock();

                if matches!(state, State::Failed | State::Terminated) {
                    break;
                }
            }
            Command::Shutdown => {
                shutdown(&mut mainloop, &mut context);
                return;
            }
        }
    }

    shutdown(&mut mainloop, &mut context);
}

/// Disconnects the context and stops the mainloop thread in the order `libpulse` requires.
fn shutdown(mainloop: &mut Mainloop, context: &mut Context) {
    mainloop.lock();
    context.disconnect();
    mainloop.unlock();
    mainloop.stop();
}
