# 1. Clone & Enter Repository
git clone https://github.com/kneu-vibeathon/campusflow-protocol.git
cd campusflow-protocol

# 2. Install Dependencies
yarn install

# 3. Build Anchor Rust Program
anchor build

# 4. Run Full QA Test Suite against Localnet
anchor test

# 5. Deploy to Solana Devnet
solana config set --url https://api.devnet.solana.com
solana airdrop 2
anchor deploy --provider.cluster devnet