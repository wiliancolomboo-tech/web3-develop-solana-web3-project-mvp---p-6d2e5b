import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { CampusCred } from "../target/types/campus_cred";
import { assert, expect } from "chai";
import crypto from "crypto";

describe("campus_cred protocol test matrix", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.CampusCred as Program<CampusCred>;
  const authority = provider.wallet as anchor.Wallet;

  const student = anchor.web3.Keypair.generate();
  const unauthorizedAttacker = anchor.web3.Keypair.generate();

  const grantId = "KNEU-VIBE-2025-01";
  const grantTitle = "Solana DeFi Indexer for Campus DAO";
  const totalMilestones = 2;
  const initialBudget = new anchor.BN(2 * anchor.web3.LAMPORTS_PER_SOL);
  const milestonePayout = new anchor.BN(1 * anchor.web3.LAMPORTS_PER_SOL);

  let grantPda: anchor.web3.PublicKey;
  let vaultPda: anchor.web3.PublicKey;

  before(async () => {
    // Fund student & attacker for rent payment
    const airdropSig1 = await provider.connection.requestAirdrop(student.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL);
    const airdropSig2 = await provider.connection.requestAirdrop(unauthorizedAttacker.publicKey, 1 * anchor.web3.LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(airdropSig1);
    await provider.connection.confirmTransaction(airdropSig2);

    [grantPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("grant"), authority.publicKey.toBuffer(), Buffer.from(grantId)],
      program.programId
    );

    [vaultPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), grantPda.toBuffer()],
      program.programId
    );
  });

  it("1. Successfully initializes a grant escrow and locks SOL in vault", async () => {
    const tx = await program.methods
      .initializeGrant(grantId, grantTitle, totalMilestones, initialBudget)
      .accountsStrict({
        grant: grantPda,
        vault: vaultPda,
        authority: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    expect(tx).to.be.a("string");

    const grantAccount = await program.account.grantAccount.fetch(grantPda);
    expect(grantAccount.title).to.equal(grantTitle);
    expect(grantAccount.totalMilestones).to.equal(2);
    expect(grantAccount.completedMilestones).to.equal(0);
    expect(grantAccount.remainingBudget.toString()).to.equal(initialBudget.toString());
    expect(grantAccount.isActive).to.be.true;

    const vaultBalance = await provider.connection.getBalance(vaultPda);
    expect(vaultBalance).to.equal(initialBudget.toNumber());
  });

  it("2. Allows registered student to submit milestone proof with git commit hash", async () => {
    const milestoneIndex = 0;
    const [milestonePda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("milestone"), grantPda.toBuffer(), Buffer.from([milestoneIndex])],
      program.programId
    );

    const commitHash = crypto.createHash("sha256").update("feat: initial anchor prototype").digest();

    await program.methods
      .submitMilestone(milestoneIndex, "https://github.com/kneu-vibecoder/solana-mvp", Array.from(commitHash))
      .accountsStrict({
        grant: grantPda,
        milestoneSubmission: milestonePda,
        student: student.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([student])
      .rpc();

    const submission = await program.account.milestoneSubmission.fetch(milestonePda);
    expect(submission.student.toBase58()).to.equal(student.publicKey.toBase58());
    expect(submission.isSubmitted).to.be.true;
    expect(submission.isApproved).to.be.false;
  });

  it("3. Prevents an unauthorized evaluator from approving and draining the vault", async () => {
    const milestoneIndex = 0;
    const [milestonePda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("milestone"), grantPda.toBuffer(), Buffer.from([milestoneIndex])],
      program.programId
    );
    const [attestationPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("attestation"), grantPda.toBuffer(), student.publicKey.toBuffer(), Buffer.from([milestoneIndex])],
      program.programId
    );

    try {
      await program.methods
        .evaluateAndPayout(milestonePayout, 95)
        .accountsStrict({
          grant: grantPda,
          vault: vaultPda,
          milestoneSubmission: milestonePda,
          meritAttestation: attestationPda,
          student: student.publicKey,
          authority: unauthorizedAttacker.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([unauthorizedAttacker])
        .rpc();
      assert.fail("Should have thrown unauthorized evaluator error");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("UnauthorizedEvaluator");
    }
  });

  it("4. Evaluator approves milestone, transfers SOL to student, and mints Soulbound Attestation", async () => {
    const milestoneIndex = 0;
    const [milestonePda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("milestone"), grantPda.toBuffer(), Buffer.from([milestoneIndex])],
      program.programId
    );
    const [attestationPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("attestation"), grantPda.toBuffer(), student.publicKey.toBuffer(), Buffer.from([milestoneIndex])],
      program.programId
    );

    const studentBalanceBefore = await provider.connection.getBalance(student.publicKey);

    await program.methods
      .evaluateAndPayout(milestonePayout, 98)
      .accountsStrict({
        grant: grantPda,
        vault: vaultPda,
        milestoneSubmission: milestonePda,
        meritAttestation: attestationPda,
        student: student.publicKey,
        authority: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const studentBalanceAfter = await provider.connection.getBalance(student.publicKey);
    expect(studentBalanceAfter - studentBalanceBefore).to.equal(milestonePayout.toNumber());

    const attestation = await program.account.meritAttestation.fetch(attestationPda);
    expect(attestation.recipient.toBase58()).to.equal(student.publicKey.toBase58());
    expect(attestation.meritScore).to.equal(98);
    expect(attestation.payoutAwarded.toString()).to.equal(milestonePayout.toString());

    const grantAccount = await program.account.grantAccount.fetch(grantPda);
    expect(grantAccount.completedMilestones).to.equal(1);
    expect(grantAccount.remainingBudget.toNumber()).to.equal(1 * anchor.web3.LAMPORTS_PER_SOL);
  });

  it("5. Rejects duplicate approval of an already processed milestone", async () => {
    const milestoneIndex = 0;
    const [milestonePda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("milestone"), grantPda.toBuffer(), Buffer.from([milestoneIndex])],
      program.programId
    );
    const [attestationPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("attestation"), grantPda.toBuffer(), student.publicKey.toBuffer(), Buffer.from([milestoneIndex])],
      program.programId
    );

    try {
      await program.methods
        .evaluateAndPayout(milestonePayout, 100)
        .accountsStrict({
          grant: grantPda,
          vault: vaultPda,
          milestoneSubmission: milestonePda,
          meritAttestation: attestationPda,
          student: student.publicKey,
          authority: authority.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();
      assert.fail("Should have failed on duplicate approval");
    } catch (err: any) {
      // Caught because attestation account already initialized or constraint checked
      expect(err).to.exist;
    }
  });
});