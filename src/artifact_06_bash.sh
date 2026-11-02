# 1. Build Optimized BPF Program
anchor build

# 2. Synchronize Program ID with Keypair
solana-keygen pubkey target/deploy/edustream-keypair.json
# Ensure ID matches declare_id!() in Rust

# 3. Deploy to Solana Devnet / Testnet
solana config set --url https://api.devnet.solana.com
solana airdrop 2
anchor deploy --provider.cluster devnet

# 4. Run Complete Integration Suite
anchor test --provider.cluster devnet