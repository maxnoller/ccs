# Claude Code Sandbox Container
# Run Claude Code with --dangerously-skip-permissions safely in isolation

FROM node:22-slim

# Install essential tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    curl \
    ca-certificates \
    openssh-client \
    && rm -rf /var/lib/apt/lists/*

# Install Claude Code globally
RUN npm install -g @anthropic-ai/claude-code

# Use existing node user (UID 1000) and rename to claude for clarity
RUN usermod -l claude -d /home/claude -m node \
    && groupmod -n claude node

# Create necessary directories with correct permissions
RUN mkdir -p /home/claude/.claude /workspace \
    && chown -R claude:claude /home/claude /workspace

# Switch to non-root user
USER claude
WORKDIR /workspace

# Set environment variables
ENV HOME=/home/claude
ENV NODE_ENV=production

# Entrypoint runs Claude Code with dangerous permissions flag
# This is safe because we're isolated in a container
ENTRYPOINT ["claude", "--dangerously-skip-permissions"]
