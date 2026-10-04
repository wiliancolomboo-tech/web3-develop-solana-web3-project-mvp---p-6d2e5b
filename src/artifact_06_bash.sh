# Clone and enter workspace
git clone https://github.com/aegisedu/aegis-solana-mvp.git
cd aegis-solana-mvp

# Build Anchor zero-dependency bytecode
anchor build

# Switch to Solana Devnet
solana config set --url https://api.devnet.solana.com

# Deploy Program to Devnet
anchor deploy --provider.cluster devnet