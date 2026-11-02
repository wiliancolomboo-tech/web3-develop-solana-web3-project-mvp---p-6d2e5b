# 1. Install dependencies
yarn install

# 2. Build Anchor Smart Program
anchor build

# 3. Synchronize program ID keypair
solana address -k target/deploy/scholar_stream-keypair.json
# (Paste output into declare_id! inside programs/scholar_stream/src/lib.rs)

# 4. Deploy to Solana Devnet
solana config set --url https://api.devnet.solana.com
anchor deploy --provider.cluster devnet

# 5. Run full end-to-end deterministic test suite
anchor test