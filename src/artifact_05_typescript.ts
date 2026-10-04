import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { AegisEdu } from "../target/types/aegis_edu";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";
import crypto from "crypto";

describe("AegisEdu Protocol Test Suite", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.AegisEdu as Program<AegisEdu>;

  const facultyAuthority = Keypair.generate();
  const studentResearcher = Keypair.generate();
  const grantTitle = "AI-Economics-KNEU-2025";
  const totalMilestones = 2;
  const totalBudget = new anchor.BN(2 * LAMPORTS_PER_SOL);

  let grantPda: PublicKey;
  let vaultPda: PublicKey;

  before(async () => {
    // Fund test accounts
    const airdrop1 = await provider.connection.requestAirdrop(
      facultyAuthority.publicKey,
      5 * LAMPORTS_PER_SOL
    );
    const airdrop2 = await provider.connection.requestAirdrop(
      studentResearcher.publicKey,
      2 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdrop1);
    await provider.connection.confirmTransaction(airdrop2);

    [grantPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("grant"),
        facultyAuthority.publicKey.toBuffer(),
        Buffer.from(grantTitle),
      ],
      program.programId
    );

    [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), grantPda.toBuffer()],
      program.programId
    );
  });

  it("1. Successfully initializes a grant with deterministic PDA", async () => {
    await program.methods
      .initializeGrant(grantTitle, totalMilestones, totalBudget)
      .accounts({
        authority: facultyAuthority.publicKey,
        recipient: studentResearcher.publicKey,
        grant: grantPda,
        vault: vaultPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([facultyAuthority])
      .rpc();

    const grantAccount = await program.account.grant.fetch(grantPda);
    expect(grantAccount.authority.toBase58()).to.equal(facultyAuthority.publicKey.toBase58());
    expect(grantAccount.recipient.toBase58()).to.equal(studentResearcher.publicKey.toBase58());
    expect(grantAccount.totalMilestones).to.equal(2);
    expect(grantAccount.completedMilestones).to.equal(0);
    expect(grantAccount.isActive).to.be.true;
  });

  it("2. Accepts escrow funding from sponsor to Vault PDA", async () => {
    await program.methods
      .fundGrant(totalBudget)
      .accounts({
        funder: facultyAuthority.publicKey,
        grant: grantPda,
        vault: vaultPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([facultyAuthority])
      .rpc();

    const vaultBalance = await provider.connection.getBalance(vaultPda);
    expect(vaultBalance).to.equal(2 * LAMPORTS_PER_SOL);

    const grantAccount = await program.account.grant.fetch(grantPda);
    expect(grantAccount.fundedAmount.toString()).to.equal(totalBudget.toString());
  });

  it("3. Rejects milestone submission from an unauthorized account", async () => {
    const maliciousAttacker = Keypair.generate();
    const mockHash = Array.from(crypto.createHash("sha256").update("draft_1").digest());

    const [milestonePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("milestone"), grantPda.toBuffer(), Buffer.from([0])],
      program.programId
    );

    try {
      await program.methods
        .submitMilestone(0, mockHash)
        .accounts({
          submitter: maliciousAttacker.publicKey,
          grant: grantPda,
          milestone: milestonePda,
          systemProgram: SystemProgram.programId,
        })
        .signers([maliciousAttacker])
        .rpc();
      expect.fail("Should have thrown UnauthorizedRecipient error");
    } catch (err: any) {
      expect(err.toString()).to.include("UnauthorizedRecipient");
    }
  });

  it("4. Student submits valid milestone deliverable proof", async () => {
    const mockHash = Array.from(crypto.createHash("sha256").update("paper_chapter_1").digest());
    const [milestonePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("milestone"), grantPda.toBuffer(), Buffer.from([0])],
      program.programId
    );

    await program.methods
      .submitMilestone(0, mockHash)
      .accounts({
        submitter: studentResearcher.publicKey,
        grant: grantPda,
        milestone: milestonePda,
        systemProgram: SystemProgram.programId,
      })
      .signers([studentResearcher])
      .rpc();

    const milestoneAccount = await program.account.milestone.fetch(milestonePda);
    expect(milestoneAccount.isCompleted).to.be.false;
    expect(milestoneAccount.index).to.equal(0);
  });

  it("5. Faculty authority approves milestone & transfers lamports directly to student", async () => {
    const [milestonePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("milestone"), grantPda.toBuffer(), Buffer.from([0])],
      program.programId
    );

    const initialStudentBalance = await provider.connection.getBalance(
      studentResearcher.publicKey
    );

    await program.methods
      .approveMilestone(0)
      .accounts({
        authority: facultyAuthority.publicKey,
        recipient: studentResearcher.publicKey,
        grant: grantPda,
        milestone: milestonePda,
        vault: vaultPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([facultyAuthority])
      .rpc();

    const postStudentBalance = await provider.connection.getBalance(
      studentResearcher.publicKey
    );
    expect(postStudentBalance - initialStudentBalance).to.equal(1 * LAMPORTS_PER_SOL);

    const grantAccount = await program.account.grant.fetch(grantPda);
    expect(grantAccount.completedMilestones).to.equal(1);
    expect(grantAccount.releasedAmount.toString()).to.equal(
      new anchor.BN(1 * LAMPORTS_PER_SOL).toString()
    );
  });

  it("6. Prevents double-approval of an already executed milestone", async () => {
    const [milestonePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("milestone"), grantPda.toBuffer(), Buffer.from([0])],
      program.programId
    );

    try {
      await program.methods
        .approveMilestone(0)
        .accounts({
          authority: facultyAuthority.publicKey,
          recipient: studentResearcher.publicKey,
          grant: grantPda,
          milestone: milestonePda,
          vault: vaultPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([facultyAuthority])
        .rpc();
      expect.fail("Should have failed on duplicate approval check");
    } catch (err: any) {
      expect(err.toString()).to.include("InvalidMilestoneOrder");
    }
  });
});