# Launch containerized enterprise listener
docker compose up -d --build

# Inspect real-time stream logs
docker logs -f campuscred_agent