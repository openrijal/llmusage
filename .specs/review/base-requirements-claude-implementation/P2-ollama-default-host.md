---
id: P2-ollama-default-host
priority: P2
status: completed
file: src/collectors/mod.rs:49-52
---

# Fall back to the default Ollama host when unset

## Problem

`print_config` and the PRD describe `http://localhost:11434` as the default
Ollama endpoint, but `get_collectors` only creates an Ollama collector when
`ollama_host` is explicitly configured. On a standard local Ollama setup,
`llmusage sync` silently skips Ollama entirely.

## Fix

Use `cfg.ollama_host` if set, otherwise fall back to `http://localhost:11434`
as the default, matching the documented behavior.
