use hyper::{Body, Request, Response};
use std::convert::Infallible;
use hyper::service::{make_service_fn, service_fn};
use futures_util::{StreamExt, SinkExt};
use std::io::Read;
use futures::Sink;
use tokio_tungstenite::WebSocketStream;
use std::sync::Arc;
use std::sync::Mutex;
use hyper::upgrade::Upgraded;
use std::collections::{HashMap, HashSet};

struct Apioform {
	sink: futures_util::stream::SplitSink<WebSocketStream<Upgraded>, tungstenite::Message>,
	id: usize
//	strs: Arc<Mutex<Vec<String>>>
}

async fn handle(mut req: Request<Body>, apioforms: Arc<Mutex<Vec<Apioform>>>, tx: tokio::sync::mpsc::Sender<(usize, Option<String>)>, api: usize) -> Result<Response<Body>, std::convert::Infallible> {
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
						tx.send((api, Some(message))).await.unwrap();
					}
					tx.send((api, None)).await.unwrap();
					println!("ws connection closed");
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

struct WorldData {
	points: HashMap<usize, (String, String)>,
	pi: usize, // key for hashmap, point id, incremented after each placement
	// the amount of connections between two points
	// the idea is that at most two polygons can border a line
	// the u8 should either be 1 or 2
	lines: HashMap<[usize; 2], u8>,
	tris: HashSet<[usize; 3]>,
	quads: HashSet<[usize; 4]>,
}

impl WorldData {
	pub fn new() -> WorldData {
		WorldData {
			points: HashMap::new(),
			lines: HashMap::new(),
			tris: HashSet::new(),
			quads: HashSet::new(),
			pi: 0
		}
	}
}

fn place_point(wd: &mut WorldData, tokens: &mut Vec<String>, x: &str, y: &str) {
	wd.points.insert(wd.pi, (x.to_string(), y.to_string()));
	tokens.push("placePoint".to_string());
	tokens.push(wd.pi.to_string());
	wd.pi += 1;
	tokens.push(x.to_string());
	tokens.push(y.to_string());
}

fn points_to_line(wd: &mut WorldData, tokens: &mut Vec<String>, pia: usize, pib: usize) {
	let mut v = vec![pia, pib]; v.sort();
	if let Some(l) = wd.lines.get(&[v[0], v[1]]) {
		wd.lines.insert([v[0], v[1]], l + 1);
	} else {
		wd.lines.insert([v[0], v[1]], 1);
	}
	tokens.push("connectPoints".to_string());
	tokens.push(v[0].to_string());
	tokens.push(v[1].to_string());
}

fn points_to_tri(wd: &mut WorldData, tokens: &mut Vec<String>, pia: usize, pib: usize, pic: usize) {
	let mut v = vec![pia, pib, pic]; v.sort();
	points_to_line(wd, tokens, pia, pib);
	points_to_line(wd, tokens, pia, pic);
	points_to_line(wd, tokens, pib, pic);
	wd.tris.insert([v[0], v[1], v[2]]);
	tokens.push("makeTri".to_string());
	tokens.push(v[0].to_string());
	tokens.push(v[1].to_string());
	tokens.push(v[2].to_string());
}

fn dump_wd(wd: &mut WorldData, tokens: &mut Vec<String>) {
	tokens.push("setPoints".to_string());
	tokens.push(wd.points.len().to_string());
	for (pi, xy) in &wd.points {
		tokens.push(pi.to_string());
		tokens.push(xy.0.to_string());
		tokens.push(xy.1.to_string());
	}
	tokens.push("setLines".to_string());
	tokens.push(wd.lines.len().to_string());
	for (pii, lc) in &wd.lines {
		tokens.push(pii[0].to_string());
		tokens.push(pii[1].to_string());
		tokens.push(lc.to_string());
	}
	tokens.push("setTris".to_string());
	tokens.push(wd.tris.len().to_string());
	for tri in &wd.tris {
		tokens.push(tri[0].to_string());
		tokens.push(tri[1].to_string());
		tokens.push(tri[2].to_string());
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
			handle(req, apioforms.clone(), tx.clone(), *apj)
		});
		async move { Ok::<_, Infallible>(service) }
	});
	tokio::spawn(async move {
		let mut wd = WorldData::new();
		while let Some((api, message)) = rx.recv().await {
			if message.is_none() {
				// stream closed
				let mut apios = apios.lock().unwrap();
				let mut rem = false;
				for i in 0..apios.len() {
					if apios[i].id == api {
						println!("remove conn no. {}  ({}-th)", api, i);
						apios.remove(i);
						rem = true;
						break;
					}
				}
				if !rem {
					eprintln!("did not remove conn no. {}", api);
				}
				continue;
			}
			let message = message.unwrap();
			let mut tokens: Vec<&str> = message.split(' ').collect();
			// client to server:
			// placePoint () x, y
			// connectPoints () pi, pi
			// disconnPoints () pi, pi
			// lineToTri () pi, pi, x, y
			// triToQuad () pi, pi, pif, x, y
			// makeTriXY () pi, pi, x, y
			// makeTri () pi, pi, pi
			loop {
				if tokens.len() == 0 { break; }
				let cmd = tokens.remove(0);
				let mut outok: Vec<String> = Vec::new();
				match cmd {
				"placePoint" => {
					let x = tokens.remove(0);
					let y = tokens.remove(0);
					place_point(&mut wd, &mut outok, x, y);
				},
				"makeTri" => {
					let pia = tokens.remove(0).parse::<usize>().unwrap();
					let pib = tokens.remove(0).parse::<usize>().unwrap();
					let pic = tokens.remove(0).parse::<usize>().unwrap();
					points_to_tri(&mut wd, &mut outok, pia, pib, pic);
				},
				_ => {}
				};
				outok = Vec::new();
				dump_wd(&mut wd, &mut outok);
				let msg = outok.join(" ");
				let mut i: usize = 0;
				loop {
					let mut apio = {
						let mut apios = apios.lock().unwrap();
						if i >= apios.len() {
							break;
						}
						apios.remove(i)
					};
					apio.sink.send(tungstenite::Message::Text(msg.to_string())).await.unwrap();
					//if api == apio.id {
					//	apio.sink.send(tungstenite::Message::Text(selfmsg.to_string())).await.unwrap();
					//} else {
					//	apio.sink.send(tungstenite::Message::Text(awaymsg.to_string())).await.unwrap();
					//}
					{
						let mut apios = apios.lock().unwrap();
						apios.insert(i, apio);
					}
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
