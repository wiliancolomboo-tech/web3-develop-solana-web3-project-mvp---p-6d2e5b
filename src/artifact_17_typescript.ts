import express from "express";
import { Connection, PublicKey } from "@solana/web3.js";
import { config } from "./config";
import { NotificationService } from "./services/notificationService";

const app = express();
app.use(express.json());

let isHealthy = false;
let lastSlotProcessed = 0;

// Healthcheck endpoints for enterprise telemetry
app.get("/healthz", (_, res) => {
  if (isHealthy) {
    res.status(200).json({ status: "healthy", lastSlot: lastSlotProcessed, timestamp: Date.now() });
  } else {
    res.status(503).json({ status: "initializing" });
  }
});

app.get("/metrics", (_, res) => {
  res.status(200).json({
    service: "scholar-stream-daemon",
    uptimeSeconds: process.uptime(),
    memoryUsageBytes: process.memoryUsage().rss,
    lastSlotProcessed,
  });
});

async function main() {
  console.log("--------------------------------------------------");
  console.log("  ScholarStream Daemon & Enterprise Bridge Starting");
  console.log(`  Target Program ID: ${config.programId}`);
  console.log(`  RPC Connection   : ${config.solanaRpcUrl}`);
  console.log("--------------------------------------------------");

  const connection = new Connection(config.solanaRpcUrl, {
    commitment: "confirmed",
    wsEndpoint: config.solanaWsUrl,
  });

  const programPubkey = new PublicKey(config.programId);

  // Subscribe to program transaction logs
  connection.onLogs(
    programPubkey,
    async (logsContext) => {
      const { signature, err, logs } = logsContext;
      if (err) return;

      console.log(`\n[NEW TRANSACTION OBSERVED] Tx: ${signature}`);

      for (const log of logs) {
        if (log.includes("GrantInitializedEvent")) {
          await NotificationService.sendSlackAlert("New University Research Grant Initialized", {
            TxHash: signature,
            Program: programPubkey.toBase58(),
          });
          await NotificationService.sendResendEmail(
            config.adminEmail,
            "University Grant Established: ScholarStream Escrow Created",
            `<h3>New Grant Initialized on Solana</h3><p>Tx Hash: <b>${signature}</b></p><p>Funds are secured in PDA escrow.</p>`
          );
        } else if (log.includes("MilestoneApprovedEvent")) {
          await NotificationService.sendSlackAlert("Faculty Milestone Approved", {
            TxHash: signature,
            Status: "Ready for student claim",
          });
        } else if (log.includes("MilestoneClaimedEvent")) {
          await NotificationService.sendSlackAlert("Milestone Grant Funds Disbursed to Student", {
            TxHash: signature,
            Status: "Transferred from Escrow Vault",
          });
        }
      }
    },
    "confirmed"
  );

  // Keep alive slot counter
  setInterval(async () => {
    try {
      lastSlotProcessed = await connection.getSlot();
      isHealthy = true;
    } catch {
      isHealthy = false;
    }
  }, 10_000);

  app.listen(config.port, () => {
    console.log(`[HTTP TELEMETRY] Listening on port ${config.port}`);
    isHealthy = true;
  });
}

main().catch((err) => {
  console.error("Fatal startup error:", err);
  process.exit(1);
});