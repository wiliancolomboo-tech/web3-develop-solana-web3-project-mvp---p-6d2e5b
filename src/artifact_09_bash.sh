# Build Anchor binary and generate IDL
anchor build

# Sync Program ID into lib.rs and Anchor.toml
anchor keys sync

# Deploy to Solana Devnet
solana config set --url https://api.devnet.solana.com
solana airdrop 3
anchor deploy --provider.cluster devnet