# Planner Architecture

## Overview

The Planner is responsible for decomposing high-level goals or user messages into executable action plans. It produces a `Plan` — a sequence of `PlannedAction` objects — that the executor runs.

## Implementation

Two planners exist in the codebase:

| Planner | Location | Purpose |
|---|---|---|
| `brain-planner::Planner` | `brain/brain-planner/` | Hierarchical goal planning (Goal → Objectives → Milestones → Tasks) with `TaskGraph`, DAG optimization, tool selection |
| `brain-coordinator::planner::Planner` | `brain/brain-coordinator/src/planner/mod.rs` | Conversational planning — maps user message + context → flat `Plan` of `PlannedAction` |

This document covers the **conversational planner** (`brain-coordinator`), which drives the real-time cognitive loop.

## Input

The planner receives:
- `user_message: &str` — the raw observation/text
- `recent_conversation: &str` — formatted recent turns
- `memory_context: &str` — summarized world model context
- `has_prior_knowledge: bool` — whether context is non-empty

## Prompt Construction

The planner builds a prompt with:

```
Available capabilities:
- email.send: Send an email message to a recipient
- telegram.send: Send a Telegram message
- filesystem.write: Write content to a file
...

Recent conversation:
User: hello
Companion: Hi there!

Memory context:
people: Alice, Bob; projects: AI-OS

Has prior knowledge: true

User message: send email to admin@example.com about the project
```

## Output Schema

The LLM must return **only** valid JSON:

```json
{
  "actions": [
    { "type": "search_memory", "topics": ["project"], "reason": "find context" },
    { "type": "email.send", "topics": ["admin@example.com", "Project update", "..."], "reason": "user requested email" },
    { "type": "respond", "topics": [], "reason": "confirm sent" }
  ]
}
```

## Action Types

| Type | Runtime? | Description |
|---|---|---|
| `respond` | ❌ | Final natural language response (handled by LLM) |
| `search_memory` | ❌ | Query World Model for topics |
| `search_recent_conversation` | ❌ | Search conversation history |
| `ask_clarification` | ❌ | Request user clarification |
| `ignore` | ❌ | No action needed |
| `observe` | ❌ | Acknowledge information |
| `schedule` | ❌ | Schedule future action (not yet implemented) |
| `email.send` | ✅ | Dispatch via RuntimeManager |
| `telegram.send` | ✅ | Dispatch via RuntimeManager |
| `filesystem.write` | ✅ | Dispatch via RuntimeManager |
| `calendar.create_event` | ✅ | Dispatch via RuntimeManager |
| `github.create_issue` | ✅ | Dispatch via RuntimeManager |
| ... | ✅ | Any registered runtime capability |

## Execution Flow

```
Planner.plan() 
    ──▶ Plan { actions: [PlannedAction, ...] }
           │
           ▼
CognitiveLoopService.cycle()
           │
           ▼
RuntimeAwareExecutor.execute_plan(plan)
           │
           ├── In-memory actions ──▶ ActionExecutor (respond, search_memory, ...)
           └── Runtime actions ────▶ RuntimeManager.dispatch() ──▶ MCP subprocess
```

## Capability Registry

The planner queries `CapabilityRegistry::list()` to know available actions. The registry is populated at startup via:

```rust
RuntimeManager::merge_capabilities(|c| {
    registry.register(Capability {
        name: c.name,
        description: c.description,
    })
})
```

This keeps the planner's capability list **always in sync** with the real runtime.

## Fallback Behavior

If the LLM returns invalid JSON or an empty plan:
1. Log warning
2. Return default plan: `[{ "type": "respond", "reason": "parse error" }]`

## Testing

- `brain/brain-coordinator/src/planner/mod.rs` — unit tests with mocked `IntelligenceCoordinator`
- `tests/cognitive-integration/tests/pipeline.rs` — integration tests with real MCP:
  - `cognitive_mixed_plan_dispatches_runtime_only` — filters in-memory vs runtime actions
  - `full_cognitive_pipeline_observe_plan_execute_observe` — full observe→plan→execute→observe

## Future Enhancements

1. **Multi-step planning** — produce DAGs with dependencies, not just linear lists
2. **Tool selection** — `ToolSelector` chooses best capability for a requirement
3. **Plan optimization** — `PlanOptimizer` reorders for parallelism
4. **Hierarchical goals** — integrate with `brain-goals::GoalDag` for long-term projects