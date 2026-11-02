# 1. Clone & Setup Workspace
git clone https://github.com/kneu-vibeathon/eduflow-protocol.git
cd eduflow-protocol

# 2. Build Anchor Binary
anchor build

# 3. Deploy to Solana Devnet
solana config set --url https://api.devnet.solana.com
anchor deploy --provider.cluster devnet

# 4. Run QA Verification Test Suite
anchor test

# 5. Launch Enterprise Webhook Orchestrator
cp .env.example .env
docker compose up -d --build
docker logs -f eduflow-automation-engine