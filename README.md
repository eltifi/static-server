# Static Server

A standard-conformant, high-performance static file server written in Rust.

## Features

- **Dynamic Serving**: Serves multiple domains from a single container.
- **Empty 200 OK**: Strict silence policy. Returns an empty `200 OK` response for any missing file or route (effectively hiding 404s).
- **Docker Ready**: Built on Alpine Linux for a minimal footprint.

## Usage

### Directory Structure
The server expects websites to be organized by domain name under `/var/www`:

```
/var/www/
├── example.com/
│   ├── index.html
│   └── style.css
├── beta.example.com/
│   └── index.html
└── ...
```

### Running with Docker

Mount your sites to `/var/www` and expose port 80:

```bash
docker build -t static-server .
docker run -p 80:80 -v /path/to/your/sites:/var/www static-server
```

### Configuration

- `PORT`: Server port (default: 80)
- `WEB_ROOT`: Root directory for sites (default: `/var/www`)

### Behavior

1. **Host Header Routing**: The server uses the `Host` header to find the correct directory.
   - Request: `GET /style.css` with `Host: example.com`
   - Serves: `/var/www/example.com/style.css`
   
2. **Directory Index**: If a directory is requested, `index.html` is appended.
   - Request: `GET /` with `Host: example.com`
   - Serves: `/var/www/example.com/index.html`

3. **Missing Files**: If a file is not found (404), the server returns:
   - Status: `200 OK`
   - Body: `(empty)`
   - This applies to *any* error (permissions, missing file, etc.).
