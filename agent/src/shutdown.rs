use tokio::sync::broadcast;

pub fn create_shutdown_channel() -> (broadcast::Sender<()>, broadcast::Receiver<()>) {
    broadcast::channel(1)
}

pub async fn wait_for_signal(sender: broadcast::Sender<()>) {
    let _ = tokio::signal::ctrl_c().await;
    let _ = sender.send(());
}
