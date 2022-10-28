use hyper::{Body, Request, Response};
use std::convert::Infallible;
use hyper::service::{make_service_fn, service_fn};
use std::io::Read;
use futures::Sink;
use tokio_tungstenite::WebSocketStream;
use std::sync::Arc;
use std::sync::Mutex;
use futures_util::{StreamExt, SinkExt};
//use futures_util::sink::Sink;
use hyper::upgrade::Upgraded;

struct Apioform {
	sink: futures_util::stream::SplitSink<WebSocketStream<Upgraded>, tungstenite::Message>,
	id: usize
//	strs: Arc<Mutex<Vec<String>>>
}

async fn handle(mut req: Request<Body>, apioforms: Arc<Mutex<Vec<Apioform>>>, tx: tokio::sync::mpsc::Sender<(usize, String)>, api: usize) -> Result<Response<Body>, std::convert::Infallible> {
	println!("Handling function; path is {}", req.uri());
	let uri = &req.uri().path()[1..];
	let file = std::fs::File::open(uri);
	if let Ok(mut file) = file {
		let mut buf = Vec::new();
		file.read_to_end(&mut buf).unwrap();
		if uri[uri.len()-2..] == *"js" {
			Ok(Response::builder()
			 .status(200)
			 .header("Content-Type", "application/javascript")
			 .body(Body::from(buf))
			 .unwrap())
		} else {
			Ok(Response::new(Body::from(buf)))
		}
	} else if uri == "webs" {
		let headers = req.headers();
		let reqkey = &headers["Sec-WebSocket-Key"];
		let retkey = tungstenite::handshake::derive_accept_key(reqkey.as_bytes());
		tokio::spawn(async move {
			match hyper::upgrade::on(&mut req).await {
				Ok(upgraded) => {
					let mut wsock = tokio_tungstenite::WebSocketStream::from_raw_socket(upgraded, tungstenite::protocol::Role::Server, None).await;
					tokio::macros::support::Pin::new(&mut wsock).start_send(tungstenite::Message::Text("bonjour".to_string())).unwrap();
					let (sink, mut strem) = wsock.split();
//					let strs = Arc::new(Mutex::new(Vec::new()));
					{
						let apio = Apioform { sink: sink, id: api };
						let mut apioforms = apioforms.lock().unwrap();
						apioforms.push(apio);
					}
					while let Some(Ok(tungstenite::Message::Text(message))) = strem.next().await {
//						{
//							let mut strs = strs.lock().unwrap();
//							strs.push(message);
//						}
						println!("received message {}", message);
						tx.send((api, message)).await.unwrap();
						println!("tx finished");
					}
					// let mut wsock = tungstenite::WebSocket::from_raw_socket(upgraded, tungstenite::protocol::Role::Server, None);
					// wsock.write_message(tungstenite::Message::Text("bonjour".to_string())).unwrap();
					println!("upgrade succeeded");
				},
				Err(e) => eprintln!("upgrade error: {}", e)
			}
		});
		Ok(Response::builder()
		 .status(101)
		 .header("Upgrade", "websocket")
		 .header("Connection", "Upgrade")
		 .header("Sec-WebSocket-Accept", retkey)
		 .body(Body::empty())
		 .unwrap())
	} else {
		Ok(Response::builder()
		 .status(404)
		 .body(Body::from("404 Eroor"))
		 .unwrap())
	}
}

#[tokio::main]
async fn main() {
	let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));
	let apioforms: Arc<Mutex<Vec<Apioform>>> = Arc::new(Mutex::new(Vec::new()));
	let apios = apioforms.clone();
	let (tx, mut rx) = tokio::sync::mpsc::channel(100);
	let api: usize = 0;
	let api = Arc::new(Mutex::new(api));
	let make_service = make_service_fn(move |_conn| {
		let api = api.clone();
		let tx = tx.clone();
		let apioforms = apioforms.clone();
		let service = service_fn(move |req| {
			let apj = api.clone();
			let mut apj = apj.lock().unwrap();
			*apj += 1;
			println!("{}", *apj);
			handle(req, apioforms.clone(), tx.clone(), *apj)
		});
		async move { Ok::<_, Infallible>(service) }
	});
	tokio::spawn(async move {
		while let Some((api, message)) = rx.recv().await {
			let mut i: usize = 0;
			loop {
				println!("apio started");
				let mut apio = {
					let mut apios = apios.lock().unwrap();
					if i >= apios.len() {
						break;
					}
					apios.remove(i)
				};
				let message = format!("{} {} {}", api, apio.id, message);
				if api == apio.id {
					apio.sink.send(tungstenite::Message::Text("s ".to_string() + &message)).await.unwrap();
				} else {
					apio.sink.send(tungstenite::Message::Text("h ".to_string() + &message)).await.unwrap();
				}
				{
					let mut apios = apios.lock().unwrap();
					apios.insert(i, apio);
				}
				println!("apio finished");
				i += 1;
			}
			println!("resp finished");
		}
	});
	let server = hyper::Server::bind(&addr).serve(make_service);
	if let Err(e) = server.await {
		eprintln!("server error: {}", e);
	}
}

//#[tokio::main]
//async fn main() {
//	let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));
//	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
//	loop {
//		let (stream, _) = listener.accept().await.unwrap();
//		tokio::spawn(async move {
//			let http = hyper::server::conn::Http::new();
//			let service = service_fn(handle);
//			let conn = http.serve_connection(stream, service).with_upgrades();
//			if let Err(err) = conn.await {
//				eprintln!("Error serving stream, {}", err);
//			}
//			println!("Finished with stream");
//		});
//	}
//}
