use anyhow::Result;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use rayon::prelude::*;
use spdlog::prelude::*;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Instant;
use tokio::net::TcpListener;

use crate::explorer::ExplorerError;
use crate::ExplorerEntry;

pub enum QueryResult {
    Success(String),
    PathNotFound,
    NotDirectory,
}

pub struct Service;

impl Service {
    pub async fn new(address: SocketAddr, directory: PathBuf) -> Result<Self> {
        info!("Server started at {}", address);

        let listener = TcpListener::bind(address).await?;

        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let io = TokioIo::new(stream);
                let directory = directory.clone();

                tokio::spawn(async move {
                    let svc = hyper::service::service_fn(move |req| {
                        Self::handle_request(req, directory.clone())
                    });
                    let _ = http1::Builder::new().serve_connection(io, svc).await;
                });
            }
        });

        Ok(Service)
    }

    async fn handle_request(
        req: Request<hyper::body::Incoming>,
        directory: PathBuf,
    ) -> Result<Response<Full<Bytes>>, hyper::Error> {
        let full_path = directory.join(&req.uri().path()[1..]);

        let result = tokio::task::spawn_blocking(move || {
            Self::query_directory(full_path.clone()).unwrap_or_else(|_| {
                warn!("Internal error for {}", full_path.display());
                QueryResult::PathNotFound
            })
        })
        .await
        .unwrap();

        let (status, body) = match result {
            QueryResult::Success(data) => (StatusCode::OK, data),
            QueryResult::PathNotFound => {
                const MESSAGE: &str = "Path not found!";
                (StatusCode::NOT_FOUND, MESSAGE.to_string())
            }
            QueryResult::NotDirectory => {
                const MESSAGE: &str = "Not a directory!";
                (StatusCode::BAD_REQUEST, MESSAGE.to_string())
            }
        };

        let mut response = Response::new(Full::new(Bytes::from(body)));
        *response.status_mut() = status;

        if status == StatusCode::OK {
            response.headers_mut().insert(
                hyper::header::CONTENT_TYPE,
                "application/json".parse().unwrap(),
            );
        }

        Ok(response)
    }

    fn query_directory(full_path: PathBuf) -> Result<QueryResult> {
        if !full_path.exists() {
            return Ok(QueryResult::PathNotFound);
        }
        if !full_path.is_dir() {
            return Ok(QueryResult::NotDirectory);
        }

        let start_time = Instant::now();
        let mut file_list = std::fs::read_dir(&full_path)?
            .collect::<Result<Vec<_>, _>>()?
            .par_iter()
            .filter_map(|entry| match ExplorerEntry::new(entry) {
                Ok(explorer_entry) => Some(Ok(explorer_entry)),
                Err(err @ ExplorerError::MissingSymlinkTarget(_)) => {
                    info!("{}", err);
                    None
                }
                Err(err) => Some(Err(err)),
            })
            .collect::<Result<Vec<ExplorerEntry>, _>>()?;

        file_list.par_sort();
        let data_text = sonic_rs::to_string(&file_list)?;

        let elapsed = start_time.elapsed().as_micros() as f64 / 1000.0;
        debug!(
            "Response: {} items in {} took {}ms",
            file_list.len(),
            full_path.display(),
            elapsed
        );

        Ok(QueryResult::Success(data_text))
    }
}
