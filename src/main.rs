use hyper::{Body, Request, Response};
use hyper::service::service_fn;
use std::io::Read;

async fn handle(req: Request<Body>) -> Result<Response<Body>, std::convert::Infallible> {
	println!("Handling function; path is {}", req.uri());
	let file = std::fs::File::open(&req.uri().path()[1..]);
	if let Ok(mut file) = file {
		let mut buf = Vec::new();
		file.read_to_end(&mut buf).unwrap();
		Ok(Response::new(Body::from(buf)))
	} else {
		Ok(Response::new(Body::from("Hello, World")))
	}
}

#[tokio::main]
async fn main() {
	let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	loop {
		let (stream, _) = listener.accept().await.unwrap();
		tokio::spawn(async move {
			let http = hyper::server::conn::Http::new();
			if let Err(err) = http.serve_connection(stream, service_fn(handle)).with_upgrades().await {
				eprintln!("Error serving stream, {}", err);
			}
			println!("Finished with stream");
		});
	}
}

