# Example usage

```rust
use bytes::Bytes;
use futures::StreamExt;
use modular_sys::core::Modular;
use modular_sys::dll::LibraryModular;
use std::time::Duration;
use tower::service_fn;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let modular = LibraryModular::new().unwrap();
    let mut v = modular.subscribe("$.test.{}.>").unwrap();

    tokio::spawn(async move {
        while let Some(v) = v.next().await {
            info!(
                "on event from topic: {:?}, bytes: {:02X?}",
                v.0,
                v.1.as_ref()
            )
        }
    });

    for _ in 0..10 {
        info!("publishing");
        modular.publish("$.test.me.1", Bytes::from_static(&[0, 1, 2]));
        info!("published");
    }

    modular.register_module(
        "test.service",
        service_fn(|(method, body)| async move {
            info!("called {:?}", method);
            Ok(body)
        }),
    );

    let module = modular.get_module("test.service").unwrap();
    info!("invoke");
    let v = module.invoke("hello world", Bytes::new()).await;
    info!("got result");

    tokio::time::sleep(Duration::from_micros(0)).await;
}
```