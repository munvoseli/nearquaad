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
					{
						let apio = Apioform { sink: sink, id: api };
						let mut apioforms = apioforms.lock().unwrap();
						apioforms.push(apio);
					}
					while let Some(Ok(tungstenite::Message::Text(message))) = strem.next().await {
						println!("received message {}", message);
						tx.send((api, message)).await.unwrap();
						println!("tx finished");
					}
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
		let mut points: Vec<(usize, String, String)> = Vec::new();
		let mut pi: usize = 0;
		while let Some((api, message)) = rx.recv().await {
			let mut tokens: Vec<&str> = message.split(' ').collect();
			// placePoint/2
			// placePointOnLine/4
			// connectPoint/2
			loop {
				if tokens.len() == 0 { break; }
				let cmd = tokens.remove(0);
				let (selfmsg, awaymsg) = if cmd == "placePoint" {
					let x = tokens.remove(0);
					let y = tokens.remove(0);
					points.push((pi, x.to_string(), y.to_string())); pi += 1;
					(format!("pointIndex {} {} {}", pi - 1, x, y), format!("placePoint {} {} {}", pi - 1, x, y))
				} else {
					(String::new(), String::new())
				};
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
					if api == apio.id {
						apio.sink.send(tungstenite::Message::Text(selfmsg.to_string())).await.unwrap();
					} else {
						apio.sink.send(tungstenite::Message::Text(awaymsg.to_string())).await.unwrap();
					}
					{
						let mut apios = apios.lock().unwrap();
						apios.insert(i, apio);
					}
					println!("apio finished");
					i += 1;
				}
			}
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
