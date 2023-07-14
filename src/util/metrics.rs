use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::RwLock;

use crate::archiver::{ArchiverState, ARCHIVER_STATES};

use super::{dir_size, get_cache_dir};

async fn generate_metrics(state: Arc<RwLock<ArchiverState>>) -> String {
    let state = state.read().await;
    let state_metrics = ARCHIVER_STATES
        .iter()
        .map(|s| {
            format!(
                "archivebot_state{{state=\"{}\"}} {}\n",
                s,
                if state.eq(s) { 1 } else { 0 }
            )
        })
        .collect::<String>();

    let cache_dir_metrics = format!(
        "archivebot_cache_dir_size_bytes {}\n",
        if let Ok(cache_dir) = get_cache_dir().await {
            tokio::task::spawn_blocking(move || dir_size(&cache_dir).unwrap_or(0))
                .await
                .unwrap_or(0)
        } else {
            0
        }
    );

    format!("{}{}", state_metrics, cache_dir_metrics)
}

pub async fn serve_metrics_endpoint(
    addr: SocketAddr,
    mut rx: UnboundedReceiver<ArchiverState>,
) -> hyper::Result<()> {
    let state = Arc::new(RwLock::new(ArchiverState::Idle));

    let make_svc = make_service_fn(|_conn| {
        let state = state.clone();
        async {
            Ok::<_, Infallible>(service_fn(move |_req: Request<Body>| {
                let state = state.clone();
                async move {
                    let metrics_str = generate_metrics(state).await;
                    Ok::<_, Infallible>(Response::new(metrics_str))
                }
            }))
        }
    });

    let rx_listener = {
        let state = state.clone();
        async move {
            while let Some(new_state) = rx.recv().await {
                let mut state_guard = state.write().await;
                *state_guard = new_state;
            }
        }
    };

    let server = Server::bind(&addr).serve(make_svc);

    tokio::join!(server, rx_listener).0
}
