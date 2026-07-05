#![allow(clippy::print_stdout)]

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    use std::time::Duration;
    use yoke_volume::provider::VolumeProvider;
    use yoke_volume_macos::MacOsVolumeProvider;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("build tokio current-thread runtime");
    rt.block_on(async {
        let provider = MacOsVolumeProvider::new().expect("provider construction failed");
        println!("initial state: {:?}", provider.current_state());

        let mut events = provider.subscribe_events();
        loop {
            match tokio::time::timeout(Duration::from_mins(1), events.recv()).await {
                Ok(Ok(evt)) => println!("event: {evt:?}"),
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(n))) => {
                    println!(
                        "(dropped {n} events; resync via current_state: {:?})",
                        provider.current_state()
                    );
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
                Err(_) => {
                    println!("idle; state: {:?}", provider.current_state());
                }
            }
        }
    });
}
