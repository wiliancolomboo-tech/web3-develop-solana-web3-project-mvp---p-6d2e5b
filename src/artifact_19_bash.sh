# 1. Clone repository and build Solana Anchor binary
git clone https://github.com/scholarstream/scholar-stream.git
cd scholar-stream
anchor build

# 2. Extract Program ID and verify synchronization
solana address -k target/deploy/scholar_stream-keypair.json
# Ensure Declare_ID in src/lib.rs matches target keypair

# 3. Deploy to Solana Devnet
solana config set --url https://api.devnet.solana.com
solana airdrop 2
anchor deploy --provider.cluster devnet

# 4. Launch Enterprise Worker Daemon
cd client
cp .env.example .env
npm install
npm run build
npm start