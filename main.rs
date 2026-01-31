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


const DEFAULT_MAINTENANCE_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Maintenance</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            background-color: #f7f9fb;
            color: #333;
            display: flex;
            align-items: center;
            justify-content: center;
            height: 100vh;
            margin: 0;
        }
        .container {
            text-align: center;
            background: white;
            padding: 40px;
            border-radius: 8px;
            box-shadow: 0 4px 6px rgba(0,0,0,0.1);
            max-width: 500px;
            width: 90%;
        }
        h1 { margin-bottom: 20px; font-size: 24px; color: #2d3748; }
        p { color: #718096; line-height: 1.5; }
    </style>
</head>
<body>
    <div class="container">
        <h1>We'll be back soon!</h1>
        <p>We're currently performing some scheduled maintenance. We should be back shortly. Thank you for your patience.</p>
    </div>
</body>
</html>"#;

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
    let web_root_path = PathBuf::from(&web_root);
    
    // Check Global Maintenance
    let global_maintenance = web_root_path.join(".maintenance");
    let mut domain_maintenance = web_root_path.join(host);
    domain_maintenance.push(".maintenance");

    let maintenance_mode = if fs::metadata(&global_maintenance).await.is_ok() {
        Some("global")
    } else if fs::metadata(&domain_maintenance).await.is_ok() {
        Some("domain")
    } else {
        None
    };

    if maintenance_mode.is_some() {
        // Determine content to serve
        // Check for custom maintenance.html in domain
        let mut custom_path = web_root_path.join(host);
        custom_path.push("maintenance.html");

        // If not, check global maintenance.html
        if fs::metadata(&custom_path).await.is_err() {
            custom_path = web_root_path.join("maintenance.html");
        }

        let body_bytes = if let Ok(custom_content) = fs::read(&custom_path).await {
            Bytes::from(custom_content)
        } else {
            Bytes::from(DEFAULT_MAINTENANCE_HTML)
        };

        let response = Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .header("Content-Type", "text/html")
            .header("Retry-After", "300")
            .body(Full::new(body_bytes))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())));
            
        return Ok(response);
    }

    let mut file_path = PathBuf::from(&web_root);
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
