import { Connection, PublicKey } from "@solana/web3.js";
import { Resend } from "resend";
import axios from "axios";
import * as dotenv from "dotenv";

dotenv.config();

const RPC_ENDPOINT = process.env.SOLANA_RPC_URL || "https://api.devnet.solana.com";
const PROGRAM_ID = new PublicKey(
  process.env.PROGRAM_ID || "EduVibE1111111111111111111111111111111111111"
);
const RESEND_API_KEY = process.env.RESEND_API_KEY || "re_mock_api_key";
const WEBHOOK_URL = process.env.DISCORD_OR_SLACK_WEBHOOK || "";
const NOTIFY_EMAIL = process.env.SUPERVISOR_NOTIFY_EMAIL || "faculty@kneu.edu.ua";

const resend = new Resend(RESEND_API_KEY);
const connection = new Connection(RPC_ENDPOINT, "confirmed");

interface LogPayload {
  type: "GrantCreated" | "MilestoneSubmitted" | "MilestoneReleased" | "Unknown";
  data: Record<string, string>;
}

export function parseProgramLog(logLine: string): LogPayload {
  if (logLine.includes("GrantCreatedEvent")) {
    return { type: "GrantCreated", data: { raw: logLine } };
  } else if (logLine.includes("MilestoneSubmittedEvent")) {
    return { type: "MilestoneSubmitted", data: { raw: logLine } };
  } else if (logLine.includes("MilestoneReleasedEvent")) {
    return { type: "MilestoneReleased", data: { raw: logLine } };
  }
  return { type: "Unknown", data: {} };
}

export async function dispatchEnterpriseAlert(subject: string, message: string) {
  console.log(`[ALERT] ${subject} -> ${message}`);

  // 1. Dispatch Transactional Email via Resend
  if (RESEND_API_KEY && !RESEND_API_KEY.includes("mock")) {
    try {
      await resend.emails.send({
        from: "EduVibe Grants <grants@eduvibe.kneu.edu.ua>",
        to: [NOTIFY_EMAIL],
        subject: `[KNEU EduVibe] ${subject}`,
        html: `<strong>KNEU Uni Vibeathon Protocol Event</strong><p>${message}</p>`,
      });
      console.log(`[RESEND] Notification email dispatched to ${NOTIFY_EMAIL}`);
    } catch (err: any) {
      console.error(`[RESEND_ERROR] Failed to send email: ${err.message}`);
    }
  }

  // 2. Dispatch Webhook to Enterprise Channels (Slack/Discord)
  if (WEBHOOK_URL) {
    try {
      await axios.post(WEBHOOK_URL, {
        content: `🎓 **[KNEU EduVibe Protocol Event]**\n**${subject}**\n${message}`,
      });
      console.log(`[WEBHOOK] Broadcast sent to enterprise endpoint`);
    } catch (err: any) {
      console.error(`[WEBHOOK_ERROR] Failed to dispatch webhook: ${err.message}`);
    }
  }
}

export async function startEventDaemon() {
  console.log("======================================================");
  console.log(" KNEU EduVibe Enterprise Event Daemon Initiated       ");
  console.log(` Target Program: ${PROGRAM_ID.toBase58()}`);
  console.log(` RPC Endpoint:   ${RPC_ENDPOINT}`);
  console.log("======================================================");

  connection.onLogs(
    PROGRAM_ID,
    async (logs, ctx) => {
      if (logs.err) return;

      for (const log of logs.logs) {
        if (log.includes("MilestoneSubmittedEvent")) {
          await dispatchEnterpriseAlert(
            "Milestone Review Pending",
            `A student deliverable proof has been logged on-chain. Tx Signature: ${logs.signature}. Faculty review required.`
          );
        } else if (log.includes("MilestoneReleasedEvent")) {
          await dispatchEnterpriseAlert(
            "Disbursement Finalized",
            `Grant tranche successfully settled on Solana. Tx Signature: ${logs.signature}. Funds released to student account.`
          );
        } else if (log.includes("GrantCreatedEvent")) {
          await dispatchEnterpriseAlert(
            "New Grant Initialized",
            `New academic grant vault funded and active. Signature: ${logs.signature}`
          );
        }
      }
    },
    "confirmed"
  );
}

if (require.main === module) {
  startEventDaemon().catch((e) => {
    console.error("Daemon crashed:", e);
    process.exit(1);
  });
}