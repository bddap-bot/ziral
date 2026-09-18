use anyhow::{Result, ensure};
use iroh::{Endpoint, EndpointId};
use std::{cell::RefCell, time::Duration};
use wasm_bindgen::prelude::*;

thread_local! {
    static ENDPOINT: RefCell<Option<Endpoint>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub async fn upload(id: &str, bytes: &[u8]) -> Result<Vec<u8>, JsError> {
    n0_future::time::timeout(Duration::from_secs(30), exchange(id, bytes))
        .await
        .map_err(|error| JsError::new(&error.to_string()))?
        .map_err(|error| JsError::new(&error.to_string()))
}

async fn exchange(id: &str, bytes: &[u8]) -> Result<Vec<u8>> {
    ensure!(bytes.len() <= 1_000_000, "batch too large");
    let cached = ENDPOINT.with(|endpoint| endpoint.borrow().clone());
    let endpoint = match cached {
        Some(endpoint) => endpoint,
        None => {
            let endpoint = Endpoint::builder(iroh::endpoint::presets::N0)
                .bind()
                .await?;
            ENDPOINT.with(|cached| *cached.borrow_mut() = Some(endpoint.clone()));
            endpoint
        }
    };
    let connection = endpoint
        .connect(id.parse::<EndpointId>()?, b"ziral-record/1")
        .await?;
    let (mut send, mut recv) = connection.open_bi().await?;
    send.write_all(&(bytes.len() as u32).to_le_bytes()).await?;
    send.write_all(bytes).await?;
    send.finish()?;
    let mut length = [0; 4];
    recv.read_exact(&mut length).await?;
    let length = u32::from_le_bytes(length) as usize;
    ensure!(length <= 4096, "acknowledgment too large");
    let mut response = vec![0; length];
    recv.read_exact(&mut response).await?;
    Ok(response)
}
