// Add inside client/orchestrator.ts for container health checks:
import http from "http";

const server = http.createServer((req, res) => {
  if (req.url === "/health") {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ status: "healthy", uptime: process.uptime(), network: "devnet" }));
  } else {
    res.writeHead(404);
    res.end();
  }
});
server.listen(8080, () => console.log("🩺 Healthcheck HTTP service listening on port 8080"));