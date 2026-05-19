/// Block on `fut` using a fresh current-thread Tokio runtime.
///
/// # Panics
/// Panics if the Tokio runtime cannot be constructed.
pub fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
        .block_on(fut)
}
