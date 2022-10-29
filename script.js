'use strict';
console.log("h");

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

let lastClicked = [];
let curx, cury;

canvas.addEventListener("mousemove", function(e) {
	let r = this.getBoundingClientRect();
	curx = (e.clientX - r.x) / r.width;
	cury = (e.clientY - r.y) / r.height;
}, false);
addEventListener("keydown", function(e) {
	if (e.key == "t") {
		if (lastClicked.length < 3) return;
		ws.send(["makeTri", lastClicked[0], lastClicked[1], lastClicked[2]].join(" "));
	} else if (e.key == "q") {
		if (lastClicked.length < 3) return;
		ws.send(["makeQuad", lastClicked[0], lastClicked[1], lastClicked[2]].join(" "));
	}
	console.log(e);
}, false);

function draw() {
	ctx.clearRect(0, 0, canvas.width, canvas.height);
	// tris
	for (let tri of tris) {
		ctx.beginPath();
		ctx.fillStyle = "rgba(0, 0, 0, 0.5)";
		ctx.moveTo(points[tri[0]].x * canvas.width, points[tri[0]].y * canvas.height);
		ctx.lineTo(points[tri[1]].x * canvas.width, points[tri[1]].y * canvas.height);
		ctx.lineTo(points[tri[2]].x * canvas.width, points[tri[2]].y * canvas.height);
		ctx.fill();
		ctx.closePath();
	}
	// quads
	for (let quad of quads) {
		for (let i = 0; i < 4; ++i) {
			ctx.beginPath();
			ctx.fillStyle = "rgba(0, 0, 0, 0.2)";
			let j = i;
			ctx.moveTo(points[quad[j]].x * canvas.width, points[quad[j]].y * canvas.height);
			j++; j %= 4;
			ctx.lineTo(points[quad[j]].x * canvas.width, points[quad[j]].y * canvas.height);
			j++; j %= 4;
			ctx.lineTo(points[quad[j]].x * canvas.width, points[quad[j]].y * canvas.height);
			ctx.fill();
			ctx.closePath();
		}
	}
	// points
	for (let pi in points) {
		ctx.beginPath();
		ctx.fillStyle = "#000";
		let point = points[pi];
		ctx.fillRect(point.x * canvas.width - 5, point.y * canvas.height - 5, 10, 10);
		ctx.closePath();
	}
	for (let line of lines) {
		ctx.closePath();
		ctx.beginPath();
		ctx.fillStyle = line[2] == 1 || line[2] == 2 ? "#000" : "#f00";
		ctx.lineWidth = line[2] == 2 ? 2 : 5;
		ctx.moveTo(points[line[0]].x * canvas.width, points[line[0]].y * canvas.height);
		ctx.lineTo(points[line[1]].x * canvas.width, points[line[1]].y * canvas.height);
		ctx.stroke();
		ctx.closePath();
	}
}

canvas.addEventListener("mousedown", function(e) {
	let r = this.getBoundingClientRect();
	let x = (e.clientX - r.x) / r.width;
	let y = (e.clientY - r.y) / r.height;
	if (e.button == 0) {
		pendingPoints.push({x: x, y: y});
		draw();
		if (ws.OPEN) {
			ws.send(["placePoint", x, y].join(" "));
		}
	} else {
		console.log(lastClicked);
		lastClicked.unshift(xyToPoint(x, y));
		if (lastClicked.length > 3) lastClicked.pop();
	}
	console.log(e);
	e.cancelBubble = true;
	e.preventDefault();
	return false;
}, false);
canvas.addEventListener("contextmenu", function(e) {
	e.preventDefault();
	return false;
}, false);


ws.onmessage = function(e) {
	console.log(e.data);
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
