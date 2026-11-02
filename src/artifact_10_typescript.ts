import http from "http";

export function initHealthServer(port: number = 8080) {
  const server = http.createServer((req, res) => {
    if (req.url === "/healthz" && req.method === "GET") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ status: "healthy", timestamp: Date.now(), service: "campusflow-agent" }));
      return;
    }
    res.writeHead(404);
    res.end();
  });

  server.listen(port, () => {
    console.log(`🩺 Telemetry Healthcheck listening on port ${port}`);
  });
}