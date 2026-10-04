import http from "http";

const server = http.createServer((req, res) => {
  if (req.url === "/healthz") {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ status: "HEALTHY", uptime: process.uptime() }));
  } else {
    res.writeHead(404);
    res.end();
  }
});
server.listen(8080);