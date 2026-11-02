// Insert into src/index.ts for cloud uptime monitoring
import * as http from "http";

const server = http.createServer(async (req, res) => {
  if (req.url === "/healthz") {
    try {
      const slot = await connection.getSlot("confirmed");
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ status: "healthy", currentSlot: slot, uptime: process.uptime() }));
    } catch (err: any) {
      res.writeHead(503, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ status: "degraded", error: err.message }));
    }
  } else {
    res.writeHead(404);
    res.end();
  }
});

server.listen(process.env.PORT || 8080);