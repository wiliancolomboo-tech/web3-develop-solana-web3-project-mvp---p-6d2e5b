import http from "http";
import { Connection } from "@solana/web3.js";
import { logger } from "./service/EnterpriseNotifier";

const port = process.env.PORT || 8080;
const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.devnet.solana.com";
const connection = new Connection(rpcUrl, "confirmed");

const server = http.createServer(async (req, res) => {
  if (req.url === "/healthz") {
    try {
      const slot = await connection.getSlot();
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ status: "healthy", currentSlot: slot, uptime: process.uptime() }));
    } catch (e: any) {
      res.writeHead(503, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ status: "unhealthy", error: e.message }));
    }
  } else {
    res.writeHead(404);
    res.end();
  }
});

server.listen(port, () => {
  logger.info(`SolCampus Micro-Grant Engine Telemetry listening on port ${port}`);
});