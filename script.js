'use strict';
console.log("h");

let wspath = "ws://" + window.location.hostname + ":" + window.location.port + "/webs";
let ws = new WebSocket(wspath);
ws.onmessage = function(e) {
	console.log(e.data);
};
