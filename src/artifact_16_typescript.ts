import fetch from "node-fetch";
import { config } from "../config";

export class NotificationService {
  public static async sendSlackAlert(message: string, fields?: Record<string, string>): Promise<void> {
    if (!config.slackWebhookUrl.startsWith("https://")) {
      console.log(`[SLACK MOCK LOG] ${message}`, fields);
      return;
    }

    const payload = {
      text: `🎓 *ScholarStream Alert*: ${message}`,
      attachments: fields
        ? [
            {
              color: "#14F195", // Solana Green
              fields: Object.entries(fields).map(([title, value]) => ({
                title,
                value,
                short: true,
              })),
            },
          ]
        : undefined,
    };

    try {
      const response = await fetch(config.slackWebhookUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!response.ok) {
        console.error(`Slack webhook error: ${response.statusText}`);
      }
    } catch (err) {
      console.error("Failed to post to Slack:", err);
    }
  }

  public static async sendResendEmail(to: string, subject: string, htmlContent: string): Promise<void> {
    if (config.resendApiKey.startsWith("re_test_mock")) {
      console.log(`[RESEND EMAIL MOCK TO: ${to}] SUBJECT: ${subject}`);
      return;
    }

    try {
      const response = await fetch("https://api.resend.com/emails", {
        method: "POST",
        headers: {
          Authorization: `Bearer ${config.resendApiKey}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          from: "ScholarStream Grants <onboarding@resend.dev>",
          to: [to],
          subject,
          html: htmlContent,
        }),
      });
      const data = await response.json();
      console.log(`[RESEND SENT] ID: ${JSON.stringify(data)}`);
    } catch (err) {
      console.error("Failed to send Resend email:", err);
    }
  }
}