import { Command } from "commander";
import { Connection, Keypair, PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import * as fs from "fs";
import * as dotenv from "dotenv";

dotenv.config();

const program = new Command();
const RPC_ENDPOINT = process.env.SOLANA_RPC_URL || "https://api.devnet.solana.com";
const connection = new Connection(RPC_ENDPOINT, "confirmed");

function loadKeypair(path: string): Keypair {
  const secret = JSON.parse(fs.readFileSync(path, "utf-8"));
  return Keypair.fromSecretKey(new Uint8Array(secret));
}

program
  .name("eduvibe-cli")
  .description("Turnkey Admin & Student Orchestration CLI for KNEU EduVibe Protocol")
  .version("1.0.0");

program
  .command("derive-pda")
  .description("Compute Program Derived Addresses for a specific grant ID")
  .requiredOption("-i, --grant-id <number>", "Numeric grant ID")
  .requiredOption("-p, --program-id <string>", "Program ID")
  .action((cmd) => {
    const pId = new PublicKey(cmd.programId);
    const grantIdBig = new anchor.BN(cmd.grantId);
    const grantIdBuffer = grantIdBig.toArrayLike(Buffer, "le", 8);

    const [grantPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("eduvibe_grant"), grantIdBuffer],
      pId
    );
    const [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("eduvibe_vault"), grantIdBuffer],
      pId
    );

    console.log(`Grant State PDA: ${grantPda.toBase58()}`);
    console.log(`Escrow Vault PDA: ${vaultPda.toBase58()}`);
  });

program
  .command("healthcheck")
  .description("Verifies RPC connectivity and network latency")
  .action(async () => {
    try {
      const start = Date.now();
      const slot = await connection.getSlot();
      const duration = Date.now() - start;
      console.log(`[STATUS: HEALTHY] Connected to ${RPC_ENDPOINT}`);
      console.log(`Current Slot: ${slot} | Ping Latency: ${duration}ms`);
    } catch (err: any) {
      console.error(`[STATUS: UNHEALTHY] Failed to ping RPC: ${err.message}`);
      process.exit(1);
    }
  });

program.parse(process.argv);