import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ScholarStream } from "../target/types/scholar_stream";
import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import { assert, expect } from "chai";

describe("ScholarStream Smart Contract QA Matrix", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.ScholarStream as Program<ScholarStream>;

  const sponsor = Keypair.generate();
  const student = Keypair.generate();
  const supervisor = Keypair.generate();
  const maliciousActor = Keypair.generate();

  const grantId = new anchor.BN(9942);
  const milestone1Amount = new anchor.BN(0.5 * LAMPORTS_PER_SOL);
  const milestone2Amount = new anchor.BN(1.0 * LAMPORTS_PER_SOL);
  const totalGrantFunds = milestone1Amount.add(milestone2Amount);

  let grantAccountPda: PublicKey;
  let vaultPda: PublicKey;

  before(async () => {
    // Fund sponsor, student, and maliciousActor with devnet/localnet SOL
    for (const kp of [sponsor, student, maliciousActor]) {
      const airdropSig = await provider.connection.requestAirdrop(
        kp.publicKey,
        5 * LAMPORTS_PER_SOL
      );
      const latestBlockhash = await provider.connection.getLatestBlockhash();
      await provider.connection.confirmTransaction({
        signature: airdropSig,
        ...latestBlockhash,
      });
    }

    [grantAccountPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("grant"),
        sponsor.publicKey.toBuffer(),
        grantId.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );

    [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), grantAccountPda.toBuffer()],
      program.programId
    );
  });

  it("1. Initializes grant pool and locks sponsor funds into PDA vault", async () => {
    const now = Math.floor(Date.now() / 1000);
    const deadline1 = new anchor.BN(now + 3600);
    const deadline2 = new anchor.BN(now + 7200);

    const vaultInitialBal = await provider.connection.getBalance(vaultPda);
    assert.equal(vaultInitialBal, 0);

    await program.methods
      .initializeGrant(
        grantId,
        "KNEU AI-Economy Research Grant",
        [milestone1Amount, milestone2Amount],
        [deadline1, deadline2]
      )
      .accounts({
        sponsor: sponsor.publicKey,
        student: student.publicKey,
        supervisor: supervisor.publicKey,
        grantAccount: grantAccountPda,
        vault: vaultPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([sponsor])
      .rpc();

    const grantData = await program.account.grantAccount.fetch(grantAccountPda);
    assert.equal(grantData.grantId.toNumber(), 9942);
    assert.equal(grantData.totalAmount.toString(), totalGrantFunds.toString());
    assert.equal(grantData.claimedAmount.toNumber(), 0);
    assert.equal(grantData.currentMilestone, 0);
    assert.equal(grantData.milestones.length, 2);

    const vaultFinalBal = await provider.connection.getBalance(vaultPda);
    assert.equal(vaultFinalBal, totalGrantFunds.toNumber());
  });

  it("2. Rejects milestone submission from an unauthorized actor", async () => {
    try {
      await program.methods
        .submitMilestone(0, "ipfs://QmMalformedHashMockProof")
        .accounts({
          grantAccount: grantAccountPda,
          student: maliciousActor.publicKey,
        })
        .signers([maliciousActor])
        .rpc();
      assert.fail("Should have thrown unauthorized error");
    } catch (err: any) {
      expect(err.toString()).to.include("Unauthorized");
    }
  });

  it("3. Student submits Milestone 0 with research proof", async () => {
    const ipfsProof = "ipfs://QmZtmD2qt8fJv32FNFsgrwZnjffvmie894kLh9j3gW8bJq";
    await program.methods
      .submitMilestone(0, ipfsProof)
      .accounts({
        grantAccount: grantAccountPda,
        student: student.publicKey,
      })
      .signers([student])
      .rpc();

    const grantData = await program.account.grantAccount.fetch(grantAccountPda);
    assert.deepEqual(grantData.milestones[0].status, { submitted: {} });
    assert.equal(grantData.milestones[0].proofUri, ipfsProof);
  });

  it("4. Blocks premature claim prior to supervisor approval", async () => {
    try {
      await program.methods
        .claimMilestone(0)
        .accounts({
          grantAccount: grantAccountPda,
          vault: vaultPda,
          student: student.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([student])
        .rpc();
      assert.fail("Premature claim should have been rejected");
    } catch (err: any) {
      expect(err.toString()).to.include("InvalidMilestoneState");
    }
  });

  it("5. Faculty Supervisor approves Milestone 0", async () => {
    await program.methods
      .approveMilestone(0)
      .accounts({
        grantAccount: grantAccountPda,
        supervisor: supervisor.publicKey,
      })
      .signers([supervisor])
      .rpc();

    const grantData = await program.account.grantAccount.fetch(grantAccountPda);
    assert.deepEqual(grantData.milestones[0].status, { approved: {} });
  });

  it("6. Student claims funds for approved milestone and receives SOL transfer", async () => {
    const studentPreBal = await provider.connection.getBalance(student.publicKey);
    const vaultPreBal = await provider.connection.getBalance(vaultPda);

    await program.methods
      .claimMilestone(0)
      .accounts({
        grantAccount: grantAccountPda,
        vault: vaultPda,
        student: student.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([student])
      .rpc();

    const studentPostBal = await provider.connection.getBalance(student.publicKey);
    const vaultPostBal = await provider.connection.getBalance(vaultPda);

    // Verify exactly 0.5 SOL disbursed from PDA to student
    assert.equal(vaultPreBal - vaultPostBal, milestone1Amount.toNumber());
    assert.isAbove(studentPostBal, studentPreBal);

    const grantData = await program.account.grantAccount.fetch(grantAccountPda);
    assert.deepEqual(grantData.milestones[0].status, { claimed: {} });
    assert.equal(grantData.currentMilestone, 1);
    assert.equal(grantData.claimedAmount.toString(), milestone1Amount.toString());
  });

  it("7. Deterministically prevents double-claiming of milestone", async () => {
    try {
      await program.methods
        .claimMilestone(0)
        .accounts({
          grantAccount: grantAccountPda,
          vault: vaultPda,
          student: student.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([student])
        .rpc();
      assert.fail("Double-claiming must be prohibited");
    } catch (err: any) {
      // Must fail because currentMilestone has advanced to index 1
      expect(err.toString()).to.include("NonSequentialMilestone");
    }
  });
});