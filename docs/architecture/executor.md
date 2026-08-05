# Executor Architecture

## Overview

The Executor dispatches plan actions through the RuntimeManager to the real MCP subprocess. It handles both in-memory actions (respond, search_memory) and runtime-capability actions (email.send, telegram.inject_inbound, filesystem.write, etc.).

## Implementation

### brain-coordinator::planner::executor::ActionExecutor

Handles in-memory actions within the cognitive loop:

| Action Type | Handler | Description |
|---|---|---|
| `respond` | `ActionExecutor::respond()` | LLM generates natural language response |
| `search_memory` | `ActionExecutor::search_memory()` | Query World Model for entities |
| `search_recent_conversation` | `ActionExecutor::search_recent_conversation()` | Search conversation history |
| `ask_clarification` | Direct `Decision::Communicate` | Request user input |
| `ignore` | `Decision::Wait` | No action |
| `observe` | `Decision::Wait` | Acknowledge information |
| `schedule` | `Decision::Wait` | Scheduled action (placeholder) |

### cognitive-integration::RuntimeAwareExecutor

Handles runtime-capability actions by dispatching through `RuntimeManager::dispatch()`:

```rust
pub async fn execute_plan(&self, plan: &Plan) -> Vec<(String, ActionResult)> {
    for action in &plan.actions {
        if is_in_memory_action(&action.action_type) {
            continue; // handled by brain's ActionExecutor
        }
        let runtime_action = planned_to_runtime_action(action);
        if manager.has_capability(&runtime_action.capability).await {
            let result = manager.dispatch(runtime_action).await;
            results.push((action.action_type.clone(), result));
        }
    }
    results
}
```

## Execution Flow

```
Plan { actions: [PlannedAction, ...] }
    │
    ├── In-memory actions ──▶ ActionExecutor (brain-coordinator)
    │   ├── respond → LLM → Decision::Communicate
    │   ├── search_memory → WorldModelStore query
    │   └── search_recent_conversation → VecDeque scan
    │
    └── Runtime actions ──▶ RuntimeAwareExecutor (cognitive-integration)
        │
        ▼
    RuntimeManager.dispatch(action)
        │
        ▼
    OpenClawRuntime.execute(action)
        │
        ▼
    MCP stdio → ai-os-mcp-server → Plugin handler
        │
        ▼
    Plugin returns ToolOutcome → result_map → ActionResult
        │
        ▼
    Observation emitted (status_changed / inbound_message)
```

## Input Schema Mapping

The planner produces `PlannedAction { action_type, topics: Vec<String>, reason }`.
The executor maps known capabilities to their structured input:

| Capability | Input Schema |
|---|---|
| `email.send` | `{"to": str, "subject": str, "body": str}` |
| `telegram.send` | `{"chat_id": str, "text": str}` |
| `telegram.inject_inbound` | `{"chat_id": str, "sender": str, "text": str}` |
| `filesystem.write` | `{"path": str, "content": str}` |
| `filesystem.read` | `{"path": str}` |
| `calendar.create_event` | `{"date": str, "title": str, "description": str}` |
| `calendar.read` | `{"date": str}` |
| `github.create_issue` | `{"title": str, "body": str}` |
| `github.read_repository` | `{"owner": str, "repo": str}` |
| *unknown* | `{"topics": [...], "reason": "..."}` |

## Error Handling

| Error | Code | Retryable |
|---|---|---|
| Capability not found | `capability_not_found` | No |
| Dispatch transport error | `transport_error` | Yes |
| Plugin error | `tool_error` | Yes |
| Invalid input | `invalid_input` | No |
| Unknown runtime error | `internal_error` | No |

## Retry Policy

The executor does not implement retry itself. Retry is handled at the orchestrator level:
- `brain-coordinator::orchestrator::BrainOrchestrator::cognitive_tick()` detects failures
- `brain-coordinator::goal_scheduler::GoalScheduler` tracks retry counts
- `brain-goals::GoalManager::retry_goal()` re-activates failed goals

## Cancellation

Actions can be cancelled by dropping the `RuntimeManager` reference or by checking `has_capability()` before dispatch. The MCP server itself does not support mid-execution cancellation; cancelled actions are detected by the observation loop (no observation received within timeout).

## Timeout

The `Action` struct carries `deadline_ms: Option<u64>`. The runtime enforces this at the dispatch level — if the deadline has passed, the action is rejected with `ActionStatus::Failed` and code `deadline_exceeded`.

## Rollback Hooks

Not yet implemented. Future work:
1. `ActionResult` carries `rollback_action: Option<Action>` on success
2. Executor stores rollback actions in a stack
3. On failure, executor pops and dispatches rollback actions in reverse order

## Testing

All executor tests run against the real MCP subprocess:
- `cognitive_executor_dispatches_real_email` — email.send through real subprocess
- `cognitive_executor_produces_observable_side_effect` — telegram.inject_inbound produces observation
- `cognitive_mixed_plan_dispatches_runtime_only` — mixed plan filters correctly
- `cognitive_executor_handles_unknown_capability` — unknown capability returns typed failure
- `full_cognitive_pipeline_observe_plan_execute_observe` — full observe→plan→execute→observe cycle