use libpulse_binding::error::PAErr;

#[derive(Debug, thiserror::Error)]
pub enum PulseError {
    /// `libpulse` failed to allocate the threaded mainloop object.
    #[error("failed to create the PulseAudio mainloop")]
    MainLoopCreation,

    /// `libpulse` failed to allocate a context object.
    #[error("failed to create the PulseAudioContext")]
    ContextCreation,

    // Failed to spawn the OS thread to hold and handle `libpulse` objects.
    #[error("failed to create the OS worker thread: {0}")]
    ThreadCreation(#[from] std::io::Error),

    /// The background worker thread has shut down, so the request could not be send, or its result could not be
    /// delivered.
    #[error("disconnected from worker thread")]
    Disconnected,

    /// A `libpulse` C API call reported an error (connecting, starting the mainloop, etc.).
    #[error("PulseAudio error: {0}")]
    Pulse(#[from] PAErr),

    /// The context reached [`State::Failed`](libpulse_binding::context::State::Failed) or
    /// [`State::Terminated`](libpulse_binding::context::State::Terminated) while we were waiting for it to be ready.
    #[error("connection to the PulseAudio server failed or was refused")]
    ConnectionFailed,

    /// A `libpulse` operation failed by invoking the callback with it's "failure" state. This could be caused by a
    /// lookup for a non-existent index or name.
    #[error("the PulseAudio operation failed")]
    OperationFailed,
}
