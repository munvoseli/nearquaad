'use strict';

// todo:
// merge point
// delete point
// colors
// text annotations
// image annotations

let canvas = document.getElementById("canvas");
let ctx = canvas.getContext("2d");
let wspath = "ws://" + window.location.hostname + ":" + window.location.port + "/webs";
let ws = new WebSocket(wspath);
ctx.fillStyle = "#000";

let points = {};
let pendingPoints = [];
let lines = [];
let tris = [];
let quads = [];
let ngons = [];
let svgl = [];

let camera = {x: 0, y: 0, scale: 1};

function distToLine(pia, pib, x, y) {
	let {x: xa, y: ya} = points[pia];
	let {x: xb, y: yb} = points[pib];
	x -= xa;
	y -= ya;
	let xe = xb - xa;
	let ye = yb - ya;
	let det = xe * xe + ye * ye;
	let xo = xe * x + ye * y;
	let yo = -ye * x + xe * y;
	xo /= det;
	yo /= det;
	xo -= 0.5;
	xo = Math.abs(xo);
	xo = xo < 0.5 ? 0 : xo - 0.5;
	xo *= Math.sqrt(det);
	yo *= Math.sqrt(det);
	return Math.sqrt(xo * xo + yo * yo);
}

function getNearestLine(x, y) {
	let li;
	let lv;
	for (let i in lines) {
		let v = distToLine(lines[i][0], lines[i][1], x, y);
		if (v > lv) continue;
		lv = v;
		li = i;
	}
	return [li, lv];
}

function getNearestPoint(x, y) {
	let pk;
	let v;
	for (let k in points) {
		let p = points[k];
		let d = (p.x - x) ** 2 + (p.y - y) ** 2;
		if (d > v) continue;
		v = d;
		pk = k;
	}
	return [pk, Math.sqrt(v)];
}

function getNearestThing(x, y) {
	let np = getNearestPoint(x, y);
	let lp = getNearestLine(x, y);
	let res;
	console.log(np[1], lp[1]);
	if (np[1] < 0.03 * camera.scale) {
		res = ["point", np[0]];
	} else if (lp[1] < 0.01 * camera.scale) {
		res = ["line", lp[0]];
	} else {
		res = ["none"];
	}
	return res;
}

function xyToPoint(x, y) {
	let keys = [];
	for (let k in points) {
		keys.push(k);
	}
	let mind = (x - points[keys[0]].x) ** 2 + (y - points[keys[0]].y) ** 2;
	let mini = keys[0];
	for (let i = 1; i < keys.length; ++i) {
		let d = (x - points[keys[i]].x) ** 2 + (y - points[keys[i]].y) ** 2;
		if (d < mind) {
			mind = d;
			mini = keys[i];
		}
	}
	return mini;
}

let curx, cury;
let clickMode;
function asel(elid, val) {
	document.getElementById(elid).addEventListener("click", function(e) {
		clickMode = val;
	}, false);
}
asel("ia", "expand");
asel("ib", "put-tri");
asel("ic", "put-image");
asel("id", "put-text");
function setDragMode() {
	document.getElementById("ia").checked = true;
	clickMode = "expand";
}
setDragMode();

function draw() {
	ctx.resetTransform();
	ctx.clearRect(0, 0, canvas.width, canvas.height);
	setCameraTransform();
	for (let ngon of ngons) {
		ctx.beginPath();
		ctx.fillStyle = "rgba(0, 0, 0, 0.5)";
		ctx.moveTo(points[ngon[0]].x, points[ngon[0]].y);
		for (let i = 1; i < ngon.length; ++i) {
			ctx.lineTo(points[ngon[i]].x, points[ngon[i]].y);
		}
		ctx.fill();
		ctx.closePath();
	}
	// tris
//	for (let tri of tris) {
//		ctx.beginPath();
//		ctx.fillStyle = "rgba(0, 0, 0, 0.5)";
//		ctx.moveTo(points[tri[0]].x, points[tri[0]].y);
//		ctx.lineTo(points[tri[1]].x, points[tri[1]].y);
//		ctx.lineTo(points[tri[2]].x, points[tri[2]].y);
//		ctx.fill();
//		ctx.closePath();
//	}
//	// quads
//	for (let quad of quads) {
//		for (let i = 0; i < 4; ++i) {
//			ctx.beginPath();
//			ctx.fillStyle = "rgba(0, 0, 0, 0.2)";
//			let j = i;
//			ctx.moveTo(points[quad[j]].x, points[quad[j]].y);
//			j++; j %= 4;
//			ctx.lineTo(points[quad[j]].x, points[quad[j]].y);
//			j++; j %= 4;
//			ctx.lineTo(points[quad[j]].x, points[quad[j]].y);
//			ctx.fill();
//			ctx.closePath();
//		}
//	}
	// points
	let bs = 100 / camera.scale;
	for (let pi in points) {
		ctx.beginPath();
		ctx.fillStyle = "#000";
		let point = points[pi];
		ctx.fillRect(point.x - bs, point.y - bs, bs * 2, bs * 2);
		ctx.closePath();
	}
	bs /= 5;
	for (let line of lines) {
		ctx.closePath();
		ctx.beginPath();
		ctx.fillStyle = line[2] == 1 || line[2] == 2 ? "#000" : "#f00";
		ctx.lineWidth = line[2] == 2 ? 2 * bs : 5 * bs;
		ctx.moveTo(points[line[0]].x, points[line[0]].y);
		ctx.lineTo(points[line[1]].x, points[line[1]].y);
		ctx.stroke();
		ctx.closePath();
	}
	for (let line of svgl) {
		ctx.closePath();
		ctx.beginPath();
		ctx.lineWidth = 1;
		ctx.moveTo(line[0], line[1]);
		ctx.lineTo(line[2], line[3]);
		ctx.stroke();
		ctx.closePath();
	}
}

function clientToCoord(e) {
	let r = canvas.getBoundingClientRect();
	let xf = (e.clientX - r.x) * 2 / r.width - 1;
	let yf = (e.clientY - r.y) * 2 / r.height - 1;
	return [xf * camera.scale + camera.x, yf * camera.scale + camera.y];
}
function setCameraTransform() {
	// canvas should go from -c.x - c.scale to c.x + c.scale
	// cx = 0 <=> wx = c.x - c.scale
	// cx = cw <=> wx = c.x + c.scale
	// cx = (wx - c.x + c.scale) * (cw / 2 /  c.scale)
	//    = wx * (cw / 2 /  c.scale) + (c.scale - c.x) / ...
	// cx * c.scale * 2 / cw + c.x - c.scale = wx
	// (cx * 2 / cw - 1) * c.scale + c.x = wx
	let s = canvas.width / 2 / camera.scale;
	ctx.setTransform(s, 0, 0, s, (camera.scale - camera.x) * s, (camera.scale - camera.y) * s);
}
function updateCamera() {
	let x = camera.x;
	let y = camera.y;
	let s = camera.scale;
	ws.send([x - s, y - s, x + s, y + s, "getSvgWindow"].join(" "));
}
function setCamera(x, y, s) {
	camera.x = x;
	camera.y = y;
	camera.scale = s;
	updateCamera();
}

let activeNote = [];
let startStroke = [];
addEventListener("keydown", function(e) {
//	if (e.key == "t") {
//		if (lastClicked.length < 3) return;
//		ws.send([lastClicked[0], lastClicked[1], lastClicked[2], "makeTri"].join(" "));
//	} else if (e.key == "q") {
//		if (lastClicked.length < 3) return;
//		ws.send([lastClicked[0], lastClicked[1], lastClicked[2], "makeQuad"].join(" "));
	if (e.key == "a") {
		if (ws.OPEN) {
			ws.send([curx, cury, "placePoint"].join(" "));
		}
	}
	else if (e.key == "ArrowUp"  ) setCamera(camera.x, camera.y - camera.scale / 3, camera.scale);
	else if (e.key == "ArrowDown") setCamera(camera.x, camera.y + camera.scale / 3, camera.scale);
	else if (e.key == "ArrowLeft" ) setCamera(camera.x - camera.scale / 3, camera.y, camera.scale);
	else if (e.key == "ArrowRight") setCamera(camera.x + camera.scale / 3, camera.y, camera.scale);
	console.log(e.key);
}, false);
function putTriangle() {
	let str = "";
	for (let i = 0; i < 3; ++i) {
		let ang = i/3 * Math.PI * 2;
		let x = Math.cos(ang) * 20 + curx;
		let y = Math.sin(ang) * 20 + cury;
		str += x + " " + y + " placePoint ";
	}
	str += "makeTri";
	ws.send(str);
}
function startNewNote(x, y) {
	activeNote = [x, y];
	document.getElementById("note").style.display = "block";
}
document.getElementById("note-submit").addEventListener("click", function() {
	fetch("/put-note", {
		method: "POST"
	});
}, false);
document.getElementById("zoom").addEventListener("click", function(e) {
	if (camera.scale == 100) camera.scale = 200;
	else camera.scale = 100;
	updateCamera();
}, false);
canvas.addEventListener("mousedown", function(e) {
	let [x, y] = clientToCoord(e);
	startStroke = [e.button, getNearestThing(x, y)];
	if (clickMode == "expand") {
		if (startStroke[1][0] == "none") draw();
	} else if (clickMode == "put-tri") {
		putTriangle();
	} else if (clickMode == "put-image") {
	} else if (clickMode == "put-text") {
		startNewNote(x, y);
	}
	return false;
}, false);
canvas.addEventListener("mouseup", function(e) {
	let [x, y] = clientToCoord(e);
	if (clickMode == "expand") {
		if (startStroke[1][0] == "line") {
			let l = lines[startStroke[1][1]];
			let n = getNearestThing(x, y);
			if (n[0] == "point") {
				ws.send([l[0], l[1], n[1], "makeTriOrQuad"].join(" "));
			} else {
				ws.send([l[0], l[1], x, y, "placePoint", "makeTriOrQuad"].join(" "));
			}
			console.log(n);
		} else if (startStroke[1][0] == "point") {
			ws.send([startStroke[1][1], x, y, "movePoint"].join(" "));
		}
	} else if (clickMode == "put-tri") {
		setDragMode();
	} else if (clickMode == "put-image") {
		setDragMode();
	} else if (clickMode == "put-text") {
		setDragMode();
	}
}, false);
canvas.addEventListener("mousemove", function(e) {
	[curx, cury] = clientToCoord(e);
}, false);
canvas.addEventListener("contextmenu", function(e) {
	e.preventDefault();
	return false;
}, false);

ws.onopen = function() {
	setCamera(604, 246, 100);
}

ws.onmessage = function(e) {
	let terms = e.data.split(" ");
	let i = 0;
	for (;;) {
		if (i >= terms.length) break;
		let cmd = terms[i++];
		switch (cmd) {
		case "setPoints": {
			let len = parseInt(terms[i++]);
			points = [];
			for (let j = 0; j < len; ++j) {
				let pi = parseInt(terms[i++]);
				let x = parseFloat(terms[i++]);
				let y = parseFloat(terms[i++]);
				points[pi] = {x: x, y: y};
			}
		} break;
		case "setLines": {
			let len = parseInt(terms[i++]);
			lines = [];
			for (let j = 0; j < len; ++j) {
				let pia = parseInt(terms[i++]);
				let pib = parseInt(terms[i++]);
				let lc = parseInt(terms[i++]);
				lines.push([pia, pib, lc]);
			}
		} break;
		case "setTris": {
			let len = parseInt(terms[i++]);
			tris = [];
			for (let j = 0; j < len; ++j) {
				let pia = parseInt(terms[i++]);
				let pib = parseInt(terms[i++]);
				let pic = parseInt(terms[i++]);
				tris.push([pia, pib, pic]);
			}
		} break;
		case "setQuads": {
			let len = parseInt(terms[i++]);
			quads = [];
			for (let j = 0; j < len; ++j) {
				quads.push([
					parseInt(terms[i++]),
					parseInt(terms[i++]),
					parseInt(terms[i++]),
					parseInt(terms[i++]),
				]);
			}
		} break;
		case "setNgons": {
			let ct = parseInt(terms[i++]);
			ngons = [];
			for (let j = 0; j < ct; ++j) {
				let tag = parseInt(terms[i++]);
				let len = parseInt(terms[i++]);
				let ng = [];
				for (let k = 0; k < len; ++k) {
					ng.push(parseInt(terms[i++]));
				}
				ngons.push(ng);
			}
		} break;
		case "setSVG": {
			let len = parseInt(terms[i++]);
			svgl = [];
			for (let j = 0; j < len; ++j) {
				svgl.push([
					parseInt(terms[i++]),
					parseInt(terms[i++]),
					parseInt(terms[i++]),
					parseInt(terms[i++]),
				]);
			}
		} break;
		case "placePoint": {
			let pi = parseInt(terms[i++]);
			let x = parseFloat(terms[i++]);
			let y = parseFloat(terms[i++]);
			points[pi] = {x: x, y: y};
			draw();
		} break;
		case "connectPoints": {
			let pia = parseInt(terms[i++]);
			let pib = parseInt(terms[i++]);
			lines.push([pia, pib]);
			draw();
		} break;
		case "makeTri": {
			let pia = parseInt(terms[i++]);
			let pib = parseInt(terms[i++]);
			let pic = parseInt(terms[i++]);
			tris.push([pia, pib, pic]);
			draw();
		} break;
		}
		draw();
	}
};
