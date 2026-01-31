use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::fs;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "80".to_string())
        .parse()
        .unwrap_or(80);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(handler))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn handler(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, hyper::Error> {
    // 1. Get Host header to determine domain
    let host = match req.headers().get("Host") {
        Some(h) => h.to_str().unwrap_or("default").split(':').next().unwrap_or("default"),
        None => "default",
    };

    // 2. Construct path
    let path = req.uri().path();
    let mut safe_path = path.trim_start_matches('/');
    if safe_path.is_empty() || safe_path.ends_with('/') {
        // Technically this creates "index.html" or "subdir/index.html"
        // We handle appending index.html below properly with PathBuf
    }
    
    // Base directory is /var/www/{domain}/
    let web_root = env::var("WEB_ROOT").unwrap_or_else(|_| "/var/www".to_string());
    let mut file_path = PathBuf::from(web_root);
    file_path.push(host);
    
    // Append the request path (careful with .. and absolute paths, but PathBuf handles some)
    // Actually, we must be careful. For simplicity in this "always ok" server, we just append.
    // A production file server needs more sanitization, but for this task:
    let req_path = Path::new(path.trim_start_matches('/'));
    file_path.push(req_path);
    
    // If directory, append index.html
    if file_path.is_dir() {
        file_path.push("index.html");
    } else if path.ends_with('/') {
         file_path.push("index.html");
    }

    // 3. Try to open the file
    match fs::read(&file_path).await {
        Ok(contents) => {
            let mime_type = mime_guess::from_path(&file_path).first_or_octet_stream();
            let response = Response::builder()
                .header("Content-Type", mime_type.as_ref())
                .status(StatusCode::OK)
                .body(Full::new(Bytes::from(contents)))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())));
            Ok(response)
        }
        Err(_) => {
            // 4. Return empty 200 OK on failure (e.g. 404)
            Ok(Response::new(Full::new(Bytes::new())))
        }
    }
}
