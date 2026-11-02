import * as anchor from "@coral-xyz/anchor";
import { Connection, PublicKey } from "@solana/web3.js";
import axios from "axios";
import { Resend } from "resend";
import * as dotenv from "dotenv";

dotenv.config();

// Configuration & Environment Validation
const SOLANA_RPC_URL = process.env.SOLANA_RPC_URL || "https://api.devnet.solana.com";
const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID || "KNEU111111111111111111111111111111111111111");
const DISCORD_WEBHOOK_URL = process.env.DISCORD_WEBHOOK_URL || "";
const RESEND_API_KEY = process.env.RESEND_API_KEY || "";
const NOTIFICATION_EMAIL = process.env.NOTIFICATION_EMAIL || "grants@kneu.edu.ua";

const resend = RESEND_API_KEY ? new Resend(RESEND_API_KEY) : null;
const connection = new Connection(SOLANA_RPC_URL, { commitment: "confirmed", wsEndpoint: SOLANA_RPC_URL.replace("https", "wss") });

interface MilestoneNotificationPayload {
  grantId: string;
  milestoneIndex: number;
  amountSol: number;
  recipient: string;
  txSignature: string;
}

/**
 * Sends Rich Webhook to Discord / Slack channels for campus transparency
 */
async function sendWebhookAlert(payload: MilestoneNotificationPayload): Promise<void> {
  if (!DISCORD_WEBHOOK_URL) {
    console.log("[Notice] DISCORD_WEBHOOK_URL not configured. Skipping webhook broadcast.");
    return;
  }

  const embedData = {
    title: "🎓 KNEU ScholarStream: Milestone Payout Approved",
    color: 0x10b981, // Emerald Green
    fields: [
      { name: "Grant ID", value: `#${payload.grantId}`, inline: true },
      { name: "Milestone", value: `Step ${payload.milestoneIndex + 1}`, inline: true },
      { name: "Disbursed", value: `${payload.amountSol.toFixed(4)} SOL`, inline: true },
      { name: "Recipient", value: `\`${payload.recipient}\``, inline: false },
      { name: "Solana Explorer Proof", value: `[View Transaction](https://explorer.solana.com/tx/${payload.txSignature}?cluster=devnet)`, inline: false },
    ],
    timestamp: new Date().toISOString(),
    footer: { text: "KNEU UNI Vibeathon • Solana Escrow Engine" }
  };

  try {
    await axios.post(DISCORD_WEBHOOK_URL, { embeds: [embedData] });
    console.log(`[Webhook] Dispatched notification for Grant #${payload.grantId}`);
  } catch (err: any) {
    console.error(`[Webhook Error] Failed to dispatch webhook: ${err.message}`);
  }
}

/**
 * Sends formal academic disbursement receipt via Resend Enterprise API
 */
async function sendEnterpriseEmail(payload: MilestoneNotificationPayload): Promise<void> {
  if (!resend) {
    console.log("[Notice] Resend API key not configured. Skipping formal email receipt.");
    return;
  }

  try {
    const response = await resend.emails.send({
      from: "ScholarStream Grants <onboarding@resend.dev>",
      to: [NOTIFICATION_EMAIL],
      subject: `[KNEU Grants] Milestone ${payload.milestoneIndex + 1} Released: Grant #${payload.grantId}`,
      html: `
        <div style="font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; border: 1px solid #e2e8f0; border-radius: 8px;">
          <h2 style="color: #0f172a;">KNEU UNI Vibeathon Milestone Verification</h2>
          <p>This automated message confirms that an approved grant milestone has been executed on the Solana blockchain.</p>
          <div style="background-color: #f8fafc; padding: 15px; border-radius: 6px; margin: 20px 0;">
            <p><strong>Grant Identifier:</strong> #${payload.grantId}</p>
            <p><strong>Milestone Stage:</strong> Index ${payload.milestoneIndex}</p>
            <p><strong>Released Amount:</strong> <b>${payload.amountSol.toFixed(4)} SOL</b></p>
            <p><strong>Beneficiary Address:</strong> <code>${payload.recipient}</code></p>
          </div>
          <p>Transaction Signature:</p>
          <p style="word-break: break-all; background: #e2e8f0; padding: 8px; font-size: 12px;">${payload.txSignature}</p>
          <p><a href="https://explorer.solana.com/tx/${payload.txSignature}?cluster=devnet" style="color: #2563eb;">View On Solana Explorer &rarr;</a></p>
          <hr style="margin-top: 30px; border: 0; border-top: 1px solid #e2e8f0;" />
          <p style="font-size: 12px; color: #64748b;">KNEU Decentralized Academic Grants Escrow • Powered by Solana Sealevel Engine</p>
        </div>
      `,
    });
    console.log(`[Email] Resend message sent successfully with ID: ${response.data?.id}`);
  } catch (err: any) {
    console.error(`[Email Error] Failed to send via Resend: ${err.message}`);
  }
}

/**
 * Main WebSockets event listener running as a non-blocking daemon
 */
export async function startScholarDaemon() {
  console.log("=======================================================");
  console.log("🚀 KNEU ScholarStream Off-Chain Automation Daemon");
  console.log(`📡 Listening on Program ID: ${PROGRAM_ID.toBase58()}`);
  console.log(`🌐 Cluster RPC: ${SOLANA_RPC_URL}`);
  console.log("=======================================================");

  connection.onLogs(
    PROGRAM_ID,
    async ({ logs, err, signature }) => {
      if (err) return;

      const logString = logs.join(" ");

      // Detect Milestone Approval & Payout Event
      if (logString.includes("MilestoneApprovedEvent")) {
        console.log(`\n[Event Detected] Milestone Approved in Tx: ${signature}`);
        
        // Simulating parsed log data (in production, use BorshCoder/Anchor EventParser)
        const mockPayload: MilestoneNotificationPayload = {
          grantId: "101",
          milestoneIndex: 0,
          amountSol: 1.5,
          recipient: "4Zw8H2B...KneuStudent",
          txSignature: signature,
        };

        await Promise.all([
          sendWebhookAlert(mockPayload),
          sendEnterpriseEmail(mockPayload)
        ]);
      }
    },
    "confirmed"
  );
}

// Direct Execution Entrypoint
if (require.main === module) {
  startScholarDaemon().catch((err) => {
    console.error("Fatal daemon crash:", err);
    process.exit(1);
  });
}