# 1. Build Solana Anchor Program
anchor build

# 2. Deploy to Devnet / Testnet
solana config set --url https://api.devnet.solana.com
solana airdrop 2
anchor deploy --provider.cluster devnet

# 3. Initialize Environment Config
cat <<EOF > client/.env
SOLANA_RPC_URL=https://api.devnet.solana.com
PROGRAM_ID=$(solana address -k target/deploy/eduvibe_protocol-keypair.json)
RESEND_API_KEY=re_test_000000000000000000000000
DISCORD_OR_SLACK_WEBHOOK=https://api.example.com/webhooks/alerts/placeholder
SUPERVISOR_NOTIFY_EMAIL=faculty@kneu.edu.ua
EOF

# 4. Run Automated QA Test Suite
anchor test

# 5. Launch Enterprise Relay Daemon via Docker Compose
docker-compose up -d --build
docker-compose logs -f