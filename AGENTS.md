# AGENTS.md

This repository follows the streambinder/unciae repository standard.
This file records the local exceptions only.

## Exceptions

- The container runtime image is gcr.io/distroless/static, not Alpine.
  The binary is static and needs no shell and no package manager.
  Distroless gives a smaller runtime surface than Alpine for this case.

## Operations

- Production runs under the imperium praeludium role on lobster.
- A deploy is a podman auto-update after the master CI publishes the image.
