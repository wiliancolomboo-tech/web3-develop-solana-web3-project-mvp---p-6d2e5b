import dotenv from "dotenv";
dotenv.config();

export interface AppConfig {
  solanaRpcUrl: string;
  solanaWsUrl: string;
  programId: string;
  resendApiKey: string;
  slackWebhookUrl: string;
  adminEmail: string;
  port: number;
}

export const config: AppConfig = {
  solanaRpcUrl: process.env.SOLANA_RPC_URL || "http://127.0.0.1:8899",
  solanaWsUrl: process.env.SOLANA_WS_URL || "ws://127.0.0.1:8900",
  programId: process.env.PROGRAM_ID || "SchStrm111111111111111111111111111111111111",
  resendApiKey: process.env.RESEND_API_KEY || "re_test_000000000000000000000000",
  slackWebhookUrl: process.env.SLACK_WEBHOOK_URL || "https://api.example.com/webhooks/alerts/placeholder",
  adminEmail: process.env.ADMIN_EMAIL || "grants@kneu.edu.ua",
  port: parseInt(process.env.PORT || "8080", 10),
};