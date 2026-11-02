import { Connection, PublicKey } from "@solana/web3.js";
import { Resend } from "resend";
import fetch from "node-fetch";
import * as dotenv from "dotenv";

dotenv.config();

const SOLANA_RPC_URL = process.env.SOLANA_RPC_URL || "https://api.devnet.solana.com";
const SOLANA_WS_URL = process.env.SOLANA_WS_URL || "wss://api.devnet.solana.com";
const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID || "CampCred11111111111111111111111111111111111");
const DISCORD_WEBHOOK_URL = process.env.DISCORD_WEBHOOK_URL || "";
const RESEND_API_KEY = process.env.RESEND_API_KEY || "";
const NOTIFICATION_EMAIL_RECIPIENT = process.env.NOTIFICATION_EMAIL_RECIPIENT || "vibecoder@kneu.edu.ua";

const resend = RESEND_API_KEY ? new Resend(RESEND_API_KEY) : null;

interface DiscordEmbedField {
  name: string;
  value: string;
  inline?: boolean;
}

async function sendDiscordNotification(title: string, description: string, fields: DiscordEmbedField[], color = 0x14f195) {
  if (!DISCORD_WEBHOOK_URL) {
    console.log(`[WARN] Discord Webhook not set. Skipping post: ${title}`);
    return;
  }

  const payload = {
    username: "CampusCred Protocol Sentinel",
    avatar_url: "https://solana.com/src/img/branding/solanaLogoMark.png",
    embeds: [
      {
        title,
        description,
        color,
        fields,
        footer: { text: "KNEU UNI Vibeathon | Powered by Solana" },
        timestamp: new Date().toISOString(),
      },
    ],
  };

  try {
    const res = await fetch(DISCORD_WEBHOOK_URL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    if (!res.ok) {
      console.error(`[ERROR] Discord Webhook responded with status ${res.status}`);
    } else {
      console.log(`[INFO] Successfully relayed alert to Discord.`);
    }
  } catch (err) {
    console.error("[ERROR] Failed to post alert to Discord:", err);
  }
}

async function sendResendEmail(subject: string, htmlContent: string) {
  if (!resend) {
    console.log(`[WARN] Resend API Key not configured. Skipping email dispatch.`);
    return;
  }

  try {
    const response = await resend.emails.send({
      from: "CampusCred Sentinel <onboarding@resend.dev>",
      to: [NOTIFICATION_EMAIL_RECIPIENT],
      subject,
      html: htmlContent,
    });
    console.log(`[INFO] Email successfully sent via Resend: ${response.data?.id}`);
  } catch (err) {
    console.error("[ERROR] Resend dispatch failure:", err);
  }
}

async function main() {
  console.log("==========================================================");
  console.log("Starting CampusCred Enterprise Orchestration Agent...");
  console.log(`Listening on Program: ${PROGRAM_ID.toBase58()}`);
  console.log(`RPC Endpoint: ${SOLANA_RPC_URL}`);
  console.log("==========================================================");

  const connection = new Connection(SOLANA_RPC_URL, {
    wsEndpoint: SOLANA_WS_URL,
    commitment: "confirmed",
  });

  connection.onLogs(
    PROGRAM_ID,
    async ({ logs, err, signature }) => {
      if (err) return;

      for (const log of logs) {
        if (log.includes("MilestoneApprovedEvent")) {
          console.log(`\n[EVENT DETECTED] MilestoneApproved in tx: ${signature}`);

          await sendDiscordNotification(
            "🎓 Academic Grant Milestone Approved & Settled!",
            `Milestone attestation confirmed on Solana Devnet. Payout dispatched immediately from escrow.`,
            [
              { name: "Transaction Signature", value: `[\`${signature.substring(0, 16)}...\`](https://explorer.solana.com/tx/${signature}?cluster=devnet)` },
              { name: "Sponsor Verification", value: "Verified & Evaluated by KNEU Academic Board", inline: true },
              { name: "Protocol Action", value: "Soulbound Merit Attestation Minted", inline: true },
            ],
            0x14f195
          );

          const emailHtml = `
            <div style="font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto; border: 1px solid #e2e8f0; border-radius: 8px; padding: 24px;">
              <h2 style="color: #10b981;">🎉 Milestone Approval Notice</h2>
              <p>Dear KNEU Student Contributor,</p>
              <p>Your academic milestone deliverable has been verified by the faculty committee and validated on Solana.</p>
              <div style="background-color: #f8fafc; border-radius: 6px; padding: 16px; margin: 20px 0;">
                <p><strong>Transaction Signature:</strong> ${signature}</p>
                <p><strong>Status:</strong> Funds Transferred & Credential Recorded</p>
                <p><strong>Explorer:</strong> <a href="https://explorer.solana.com/tx/${signature}?cluster=devnet">View on Solana Explorer</a></p>
              </div>
              <p>Keep vibecoding and building the future of Web3!</p>
              <hr style="border: none; border-top: 1px solid #e2e8f0; margin: 20px 0;" />
              <p style="font-size: 12px; color: #64748b;">CampusCred Protocol • KNEU UNI Vibeathon</p>
            </div>
          `;

          await sendResendEmail("🎓 Grant Milestone Approved: Your Escrow Bounty Has Been Disbursed!", emailHtml);
        } else if (log.includes("MilestoneSubmittedEvent")) {
          console.log(`\n[EVENT DETECTED] MilestoneSubmitted in tx: ${signature}`);
          await sendDiscordNotification(
            "🚀 New Student Deliverable Submitted!",
            `A student has submitted code and signed a commit hash for faculty evaluation.`,
            [
              { name: "Transaction", value: `[\`${signature.substring(0, 16)}...\`](https://explorer.solana.com/tx/${signature}?cluster=devnet)` },
              { name: "Next Step", value: "Faculty Jury Code Review & Proof Verification", inline: true },
            ],
            0x3b82f6
          );
        }
      }
    },
    "confirmed"
  );
}

main().catch((err) => {
  console.error("[CRITICAL] Agent crashed:", err);
  process.exit(1);
});