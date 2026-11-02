# Verify container is running and healthy
docker inspect --format='{{json .State.Health.Status}}' campuscred_agent