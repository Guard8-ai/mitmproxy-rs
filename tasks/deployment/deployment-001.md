---
id: deployment-001
title: Add Docker Container Support
status: todo
priority: medium
tags:
- deployment
dependencies:
- api-002
- backend-013
assignee: developer
created: 2025-11-29T21:24:53.468441133Z
estimate: ~
complexity: 3
area: deployment
---

# Add Docker Container Support

> **⚠️ SESSION WORKFLOW NOTICE (for AI Agents):**
>
> **This task should be completed in ONE dedicated session.**
>
> When you mark this task as `done`, you MUST:
> 1. Fill the "Session Handoff" section at the bottom with complete implementation details
> 2. Document what was changed, what runtime behavior to expect, and what dependencies were affected
> 3. Create a clear handoff for the developer/next AI agent working on dependent tasks
>
> **If this task has dependents,** the next task will be handled in a NEW session and depends on your handoff for context.

## Context
Cloud-mitmproxy deploys mitmproxy as containerized ECS tasks on AWS Fargate. To enable drop-in replacement, mitmproxy-rs needs production-ready Docker support with:
- Multi-stage build for minimal image size
- Health check integration for orchestrators
- Environment-based configuration
- Signal handling for graceful shutdown

## Objectives
- Production-ready Dockerfile with multi-stage build
- Docker Compose for local development
- Compatible with ECS/Fargate, Kubernetes, and Docker Swarm
- Configurable via environment variables

## Tasks
- [ ] Create multi-stage Dockerfile (builder + runtime)
- [ ] Add docker-compose.yml for local testing
- [ ] Configure HEALTHCHECK instruction using /health endpoint
- [ ] Support env vars: PROXY_PORT, API_PORT, LOG_LEVEL
- [ ] Add .dockerignore for efficient builds
- [ ] Document container usage in README
- [ ] Test image size < 50MB (alpine-based)

## Acceptance Criteria
✅ **Build & Size:**
- `docker build` completes successfully
- Final image < 50MB using Alpine/distroless base
- No build tools in final image

✅ **Runtime:**
- Container starts with `docker run -p 8080:8080`
- Health check passes within 10 seconds
- Graceful shutdown on SIGTERM (15s timeout)

✅ **Configuration:**
- All settings configurable via environment variables
- Sensible defaults for quick start

## Technical Notes
- Use `rust:alpine` for builder, `alpine:latest` for runtime
- Static linking with musl for portability
- Match port 8080 default to cloud-mitmproxy expectations

## Testing
- [ ] Write unit tests for new functionality
- [ ] Write integration tests if applicable
- [ ] Ensure all tests pass before marking task complete
- [ ] Consider edge cases and error conditions

## Version Control

**⚠️ CRITICAL: Always test AND run before committing!**

- [ ] **BEFORE committing**: Build, test, AND run the code to verify it works
  - Run `cargo build --release` (or `cargo build` for debug)
  - Run `cargo test` to ensure tests pass
  - **Actually run/execute the code** to verify runtime behavior
  - Fix all errors, warnings, and runtime issues
- [ ] Commit changes incrementally with clear messages
- [ ] Use descriptive commit messages that explain the "why"
- [ ] Consider creating a feature branch for complex changes
- [ ] Review changes before committing

**Testing requirements by change type:**
- Code changes: Build + test + **run the actual program/command** to verify behavior
- Bug fixes: Verify the bug is actually fixed by running the code, not just compiling
- New features: Test the feature works as intended by executing it
- Minor changes: At minimum build, check warnings, and run basic functionality

## Updates
- 2025-11-29: Task created

## Session Handoff (AI: Complete this when marking task done)
**For the next session/agent working on dependent tasks:**

### What Changed
- [Document code changes, new files, modified functions]
- [What runtime behavior is new or different]

### Causality Impact
- [What causal chains were created or modified]
- [What events trigger what other events]
- [Any async flows or timing considerations]

### Dependencies & Integration
- [What dependencies were added/changed]
- [How this integrates with existing code]
- [What other tasks/areas are affected]

### Verification & Testing
- [How to verify this works]
- [What to test when building on this]
- [Any known edge cases or limitations]

### Context for Next Task
- [What the next developer/AI should know]
- [Important decisions made and why]
- [Gotchas or non-obvious behavior]