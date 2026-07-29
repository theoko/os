---
name: system-health-check
description: >-
  Inspect kernel memory, hardware buses, and the capability table.
  Use when the agent should report whether the machine is healthy offline.
---

# System health check

1. Check kernel memory allocations and hardware devices.
2. Inspect input controllers (USB OHCI, USB Tablet, PS/2 mouse, keyboard).
3. Review active capability grants.
4. Summarize system health (standalone: no host connector to probe).
