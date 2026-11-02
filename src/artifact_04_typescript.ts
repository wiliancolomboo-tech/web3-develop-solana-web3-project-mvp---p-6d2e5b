import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ScholarStream } from "../target/types/scholar_stream";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";

describe("scholar_stream_core_suite", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.ScholarStream as Program<ScholarStream>;

  const sponsor = Keypair.generate();
  const student = Keypair.generate();
  const evaluator = Keypair.generate();
  const attacker = Keypair.generate();

  const grantId = new anchor.BN(Date.now());
  const totalFunding = new anchor.BN(2 * LAMPORTS_PER_SOL);
  const milestonesCount = 2;

  let grantAccountPda: PublicKey;
  let vaultPda: PublicKey;

  before(async () => {
    // Fund sponsor, student, and evaluator with airdrop lamports
    const sig1 = await provider.connection.requestAirdrop(sponsor.publicKey, 10 * LAMPORTS_PER_SOL);
    const sig2 = await provider.connection.requestAirdrop(student.publicKey, 1 * LAMPORTS_PER_SOL);
    const sig3 = await provider.connection.requestAirdrop(evaluator.publicKey, 1 * LAMPORTS_PER_SOL);
    const sig4 = await provider.connection.requestAirdrop(attacker.publicKey, 1 * LAMPORTS_PER_SOL);

    const latestBlockHash = await provider.connection.getLatestBlockhash();
    await provider.connection.confirmTransaction({
      signature: sig1,
      blockhash: latestBlockHash.blockhash,
      lastValidBlockHeight: latestBlockHash.lastValidBlockHeight
    });

    [grantAccountPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("scholar_grant"), grantId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );

    [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("scholar_vault"), grantId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );
  });

  it("1. Successfully initializes grant and locks SOL in escrow PDA", async () => {
    const initialSponsorBal = await provider.connection.getBalance(sponsor.publicKey);

    await program.methods
      .initializeGrant(grantId, totalFunding, milestonesCount, "https://kneu.edu.ua/grants/vibeathon-101")
      .accounts({
        grantAccount: grantAccountPda,
        vault: vaultPda,
        sponsor: sponsor.publicKey,
        student: student.publicKey,
        evaluator: evaluator.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([sponsor])
      .rpc();

    const vaultBalance = await provider.connection.getBalance(vaultPda);
    expect(vaultBalance).to.equal(totalFunding.toNumber());

    const grantAccount = await program.account.grantAccount.fetch(grantAccountPda);
    expect(grantAccount.sponsor.toBase58()).to.equal(sponsor.publicKey.toBase58());
    expect(grantAccount.student.toBase58()).to.equal(student.publicKey.toBase58());
    expect(grantAccount.amountRemaining.toNumber()).to.equal(totalFunding.toNumber());
    expect(grantAccount.milestonesCount).to.equal(2);
    expect(grantAccount.currentMilestone).to.equal(0);
  });

  it("2. Student successfully submits work proof for Milestone 0", async () => {
    const dummyProofHash = new Array(32).fill(7);

    await program.methods
      .submitMilestone(0, dummyProofHash)
      .accounts({
        grantAccount: grantAccountPda,
        student: student.publicKey,
      })
      .signers([student])
      .rpc();
  });

  it("3. Malicious actor fails to approve milestone (UnauthorizedEvaluator)", async () => {
    try {
      await program.methods
        .approveMilestone(0)
        .accounts({
          grantAccount: grantAccountPda,
          vault: vaultPda,
          evaluator: attacker.publicKey,
          student: student.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([attacker])
        .rpc();
      expect.fail("Should have thrown unauthorized error");
    } catch (err: any) {
      expect(err.error?.errorCode?.code || err.message).to.include("UnauthorizedEvaluator");
    }
  });

  it("4. Evaluator approves Milestone 0: releases 50% funds to Student", async () => {
    const studentPreBal = await provider.connection.getBalance(student.publicKey);

    await program.methods
      .approveMilestone(0)
      .accounts({
        grantAccount: grantAccountPda,
        vault: vaultPda,
        evaluator: evaluator.publicKey,
        student: student.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([evaluator])
      .rpc();

    const studentPostBal = await provider.connection.getBalance(student.publicKey);
    expect(studentPostBal - studentPreBal).to.equal(1 * LAMPORTS_PER_SOL);

    const vaultBal = await provider.connection.getBalance(vaultPda);
    expect(vaultBal).to.equal(1 * LAMPORTS_PER_SOL);

    const grantState = await program.account.grantAccount.fetch(grantAccountPda);
    expect(grantState.currentMilestone).to.equal(1);
  });

  it("5. Completes remaining milestone: vault empties and state marks Completed", async () => {
    const dummyProofHash2 = new Array(32).fill(9);

    await program.methods
      .submitMilestone(1, dummyProofHash2)
      .accounts({
        grantAccount: grantAccountPda,
        student: student.publicKey,
      })
      .signers([student])
      .rpc();

    await program.methods
      .approveMilestone(1)
      .accounts({
        grantAccount: grantAccountPda,
        vault: vaultPda,
        evaluator: evaluator.publicKey,
        student: student.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([evaluator])
      .rpc();

    const grantState = await program.account.grantAccount.fetch(grantAccountPda);
    expect(grantState.amountRemaining.toNumber()).to.equal(0);
    expect(grantState.currentMilestone).to.equal(2);
    expect(grantState.status).to.deep.equal({ completed: {} });
  });
});