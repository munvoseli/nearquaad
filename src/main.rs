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

fn parse_point(s: &str) -> (f32, f32) {
	let ci = s.find(',').unwrap();
	let x = s[0..ci].parse::<f32>().unwrap();
	let y = s[ci + 1..].parse::<f32>().unwrap();
	(x, y)
}

fn svg_to_segments(fname: &str) -> Vec<((f32, f32), (f32, f32))> {
	let mut file = std::fs::File::open(fname).unwrap();
	let mut tokens = Vec::new();
	file.read_to_end(&mut tokens).unwrap();
	let tokens = String::from_utf8(tokens).unwrap();
	let tokens = tokens.trim();
	let tokens: Vec<&str> = tokens.split(" ").collect();
	let mut segments = Vec::new();
	let mut last_point = tokens[1];
	let mut i = 0;
	while i < tokens.len() {
		if tokens[i] == "M" {
			last_point = tokens[i + 1];
			i += 2;
		} else if tokens[i] == "L" {
			segments.push((parse_point(last_point), parse_point(tokens[i + 1])));
			last_point = tokens[i + 1];
			i += 2;
		} else if tokens[i] == "Z" {
			i += 1;
		} else if tokens[i] == "A" {
			i += 5;
		} else {
			println!("{:?}", &tokens[i - 4..i]);
			println!("Unknown token type {}", tokens[i]);
			break;
		}
	}
	segments
}

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
					tx.send((api, Some("".to_string()))).await.unwrap();
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
	vsegments: Vec<((f32, f32), (f32, f32))>
}

enum FloorTypes {
	Elevator(String),
	Room(String),
	Stairs(String),
	Ramp(String),
	Floor,
}

impl WorldData {
	pub fn new() -> WorldData {
		WorldData {
			points: HashMap::new(),
			lines: HashMap::new(),
			tris: HashSet::new(),
			quads: HashSet::new(),
			pi: 0,
			vsegments: svg_to_segments("al.txt")
		}
	}
	// todo: polynomial symmetry for indexing
	// my first thought with this was to store
	//   a+b and a*b for lines, [a+b+c, ab+bc+ac, abc] for tris,
	//   and so on
	// there's a way to invert it, because roots, so it'll be a bijection
	// (since unorder)
	// but that's just one way to look at it
	// since another way is doing abc, (a+1)(b+1)(c+1), (a+2)(b+2)(c+2)...
	// but idk if that's invertible
	// i think it is, due to Linear Algebra Stuff
	// but that's like n^2 multiplications overall still
	// need to beat sort(), which is n log n
	pub fn dump(&self) -> Vec<String> {
		let mut tokens = Vec::new();
		tokens.push("setPoints".to_string());
		tokens.push(self.points.len().to_string());
		for (pi, xy) in &self.points {
			tokens.push(pi.to_string());
			tokens.push(xy.0.to_string());
			tokens.push(xy.1.to_string());
		}
		tokens.push("setLines".to_string());
		tokens.push(self.lines.len().to_string());
		for (pii, lc) in &self.lines {
			tokens.push(pii[0].to_string());
			tokens.push(pii[1].to_string());
			tokens.push(lc.to_string());
		}
		tokens.push("setTris".to_string());
		tokens.push(self.tris.len().to_string());
		for tri in &self.tris {
			tokens.push(tri[0].to_string());
			tokens.push(tri[1].to_string());
			tokens.push(tri[2].to_string());
		}
		tokens.push("setQuads".to_string());
		tokens.push(self.quads.len().to_string());
		for quad in &self.quads {
			tokens.push(quad[0].to_string());
			tokens.push(quad[1].to_string());
			tokens.push(quad[2].to_string());
			tokens.push(quad[3].to_string());
		}
		tokens
	}
}

fn place_point(wd: &mut WorldData, tokens: &mut Vec<String>, x: &str, y: &str) -> usize {
	wd.points.insert(wd.pi, (x.to_string(), y.to_string()));
	tokens.push("placePoint".to_string());
	tokens.push(wd.pi.to_string());
	wd.pi += 1;
	tokens.push(x.to_string());
	tokens.push(y.to_string());
	return wd.pi - 1;
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

fn ppp_to_quad(wd: &mut WorldData, tokens: &mut Vec<String>, pia: usize, pib: usize, pin: usize) -> bool {
	let mut l = None;
	let mut t = None;
	'lop:
	for line in &wd.lines {
		if line.0.contains(&pia) && line.0.contains(&pib) {
			for tri in &wd.tris {
				if tri.contains(&pia) && tri.contains(&pib) {
					t = Some(tri.clone());
					l = Some(line.0.clone());
					break 'lop;
				}
			}
		}
	}
	if let (Some(tri), Some(line)) = (t, l) {
		let mut points = vec![tri[0], tri[1], tri[2], pin];
		points.sort();
		wd.tris.remove(&tri);
		wd.lines.remove(&line);
		points_to_line(wd, tokens, pia, pin);
		points_to_line(wd, tokens, pib, pin);
		wd.quads.insert([points[0], points[1], points[2], points[3]]);
		true
	} else {
		false
	}
}

fn make_tri_or_quad(wd: &mut WorldData, tokens: &mut Vec<String>, pia: usize, pib: usize, pin: usize) {
	if !ppp_to_quad(wd, tokens, pia, pib, pin) {
		points_to_tri(wd, tokens, pia, pib, pin);
	}
}

enum StackBoi {
	Raw(String),
	PointId(usize)
}

fn get_point_id(a: StackBoi) -> usize {
	if let StackBoi::PointId(a) = a {
		return a;
	} else if let StackBoi::Raw(s) = a {
		return s.parse::<usize>().unwrap();
	}
	0
}
fn get_top_float_str(s: &mut Vec<StackBoi>) -> String {
	if let StackBoi::Raw(x) = s.pop().unwrap() { x.to_string() }
	else { todo!() }
}

fn point_in_box(p: (f32, f32), x0: f32, y0: f32, x2: f32, y2: f32) -> bool {
	x0 < p.0 && p.0 < x2 &&
	y0 < p.1 && p.1 < y2
}

// 1.0 1.0 placePoint
// 0 1 1.0 1.0 placePoint makeQuad
fn run_program(wd: &mut WorldData, tokens: &Vec<&str>) -> Vec<String> {
	let mut stack: Vec<StackBoi> = Vec::new();
	let mut outok = Vec::new();
	let mut resp = Vec::new();
	for token in tokens {
		match token {
		&"placePoint" => {
			let y = get_top_float_str(&mut stack);
			let x = get_top_float_str(&mut stack);
			let pi = place_point(wd, &mut outok, &x, &y);
			stack.push(StackBoi::PointId(pi));
		},
		&"makeTri" => {
			let pic = get_point_id(stack.pop().unwrap());
			let pib = get_point_id(stack.pop().unwrap());
			let pia = get_point_id(stack.pop().unwrap());
			points_to_tri(wd, &mut outok, pia, pib, pic);
		},
		&"makeQuad" => {
			let pin = get_point_id(stack.pop().unwrap());
			let pib = get_point_id(stack.pop().unwrap());
			let pia = get_point_id(stack.pop().unwrap());
			ppp_to_quad(wd, &mut outok, pia, pib, pin);
		},
		&"makeTriOrQuad" => {
			let pin = get_point_id(stack.pop().unwrap());
			let pib = get_point_id(stack.pop().unwrap());
			let pia = get_point_id(stack.pop().unwrap());
			make_tri_or_quad(wd, &mut outok, pia, pib, pin);
		},
		&"getSvgWindow" => {
			let y2 = get_top_float_str(&mut stack).parse::<f32>().unwrap();
			let x2 = get_top_float_str(&mut stack).parse::<f32>().unwrap();
			let y0 = get_top_float_str(&mut stack).parse::<f32>().unwrap();
			let x0 = get_top_float_str(&mut stack).parse::<f32>().unwrap();
			let mut indices = Vec::new();
			{
				let mut i = 0;
				while i < wd.vsegments.len() {
					let s = &wd.vsegments[i];
					if point_in_box(s.0, x0, y0, x2, y2) || point_in_box(s.1, x0, y0, x2, y2) {
						indices.push(i);
					}
					i += 1;
				}
			}
			resp.push(format!("setSVG {}", indices.len()));
			for i in indices {
				let s = &wd.vsegments[i];
				resp.push(format!("{} {} {} {}", s.0.0, s.0.1, s.1.0, s.1.1));
			}
		},
		raw => {
			stack.push(StackBoi::Raw(raw.to_string()));
		}
		}
	}
	resp
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
			let tokens: Vec<&str> = message.split(' ').collect();
			let selfmsg = run_program(&mut wd, &tokens).join(" ");
			let msg = wd.dump().join(" ");
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
				println!("{} {}, {}", api, apio.id, api == apio.id);
				if api == apio.id {
					println!("{}", selfmsg.len());
					apio.sink.send(tungstenite::Message::Text(selfmsg.to_string())).await.unwrap();
				}
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
