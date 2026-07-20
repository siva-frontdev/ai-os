# Brain Platform — Configuration Reference

All Brain Platform configuration lives under the `[brain]` key in `brain.toml` (loaded at startup, hot-reloaded where noted). This document describes every key, its type, default, and the subsystem that consumes it.

---

## Loading Rules

- File: `configs/brain.toml` (workspace root), overridable via `BRAIN_CONFIG_PATH` environment variable.
- Format: standard TOML. No custom parsers.
- Hot-reload: sections marked [Hot-reload: ✓] are observable by the relevant subsystem via the `policy.rule.changed` event or a dedicated reload mechanism. Other sections require a full platform restart.
- Environment overrides: any key can be overridden with `BRAIN_<SECTION>_<KEY>` (uppercase, dots replaced with underscores).

---

## Top-Level Options

| Key | Type | Default | Description |
|---|---|---|---|
| `max_active_goals` | `usize` | `3` | Maximum goals running concurrently |
| `goal_scheduling` | `string` | `"priority"` | `priority` / `round_robin` / `fair_share` |
| `goal_monitoring_interval_secs` | `u64` | `5` | How often GoalManager polls for ready goals |
| `loop_tick_timeout_ms` | `u64` | `5000` | Maximum wall-clock time per Coordinator tick before yielding |

---

## [brain.budget] — Cognitive Budget Defaults

Applied when no per-goal budget is supplied. Each new goal receives a `CognitiveBudget` initialised from these defaults.

| Key | Type | Default | Description |
|---|---|---|---|
| `default_max_thinking_time_ms` | `u64` | `30000` | Maximum wall-clock per cognitive cycle |
| `default_max_iterations` | `usize` | `10` | Loop iterations before forced termination |
| `default_max_reasoning_depth` | `usize` | `5` | Chain-of-thought steps |
| `default_max_branches` | `usize` | `3` | Parallel reasoning branches |
| `default_max_model_calls` | `usize` | `5` | LLM invocations per cycle |
| `default_max_token_budget` | `u32` | `8192` | Tokens across all model calls |
| `default_max_cost_budget_cents` | `u64` | `50` | USD cents ceiling per cycle |
| `priority_weight` | `f64` | `1.0` | Multiplier applied to high-priority goal budgets |

**Hot-reload:** No. Budget defaults are read once at platform start. Per-goal overrides come from the originating event or API call.

---

## [brain.reasoning] — Reasoner Configuration

| Key | Type | Default | Description |
|---|---|---|---|
| `max_plan_steps` | `usize` | `50` | Hard ceiling on plan graph size |
| `retry_limit` | `usize` | `3` | Retry attempts for model calls before degraded mode |
| `replan_strategy_order` | `[string]` | `["means_ends", "forward_chaining", "backward_chaining", "template_match"]` | Fallback strategies tried in order on planning failure |
| `hypothesis_confidence_threshold` | `f64` | `0.6` | Minimum confidence to consider a hypothesis viable |
| `chain_of_thought_max_length` | `usize` | `2048` | Max tokens per CoT prompt segment |
| `risk_acceptance_threshold` | `f64` | `0.8` | Above this value, risk analysis is skipped (high-confidence hypotheses) |

---

## [brain.decision] — Decision Maker Configuration

| Key | Type | Default | Description |
|---|---|---|---|
| `default_strategy` | `string` | `"utility_maximize"` | `utility_maximize` / `confidence_maximize` / `risk_minimize` / `cost_minimize` |
| `confidence_threshold` | `f64` | `0.7` | Minimum confidence to accept a decision without escalation |
| `utility_weights` | `table` | see below | Weight per utility dimension |
| `conflict_resolution_policy` | `string` | `"escalate"` | `escalate` / `auto_select_top` / `defer` |
| `explanation_detail_level` | `string` | `"standard"` | `minimal` / `standard` / `verbose` |

**`[brain.decision.utility_weights]` default values:**

| Dimension | Default |
|---|---|
| `cost` | `0.3` |
| `benefit` | `0.4` |
| `risk` | `0.2` |
| `speed` | `0.05` |
| `resource` | `0.05` |

---

## [brain.policy] — Policy Engine Configuration

| Key | Type | Default | Description |
|---|---|---|---|
| `rulesets_path` | `string` | `"configs/policy/rulesets/"` | Directory containing `.json` ruleset files |
| `guard_rails_path` | `string` | `"configs/policy/guard_rails.json"` | Path to guard-rail definitions |
| `rule_reload_interval_secs` | `u64` | `60` | How often PolicyLoader polls for file changes |
| `guard_rails_enabled` | `bool` | `true` | Master switch for guard-rail enforcement |
| `evaluation_order` | `string` | `"prioritized"` | `sequential` / `prioritized` / `parallel` |
| `max_violations_before_block` | `usize` | `1` | Number of `Critical` violations that cause rejection |
| `audit_log_policy_decisions` | `bool` | `true` | Emit `brain.policy.evaluated` for every evaluation |

**Hot-reload:** ✓ PolicyLoader watches `rulesets_path` and `guard_rails_path`. Changes emit `policy.rule.changed` events. In-flight plans are not re-evaluated; newly created plans use updated rules.

---

## [brain.goals] — Goal Management Configuration

| Key | Type | Default | Description |
|---|---|---|---|
| `persistence_interval_secs` | `u64` | `30` | How often active goals are checkpointed to Memory |
| `dependency_check_on_activate` | `bool` | `true` | Re-validate dependencies when a goal becomes Active |
| `duplicate_detection_window_secs` | `u64` | `300` | Goals with identical signatures within this window are merged |
| `max_goal_hierarchy_depth` | `usize` | `10` | Maximum parent → child chain length |
| `auto_cancel_on_parent_fail` | `bool` | `true` | Children are cancelled when their parent fails |
| `default_priority` | `string` | `"normal"` | Priority for goals that do not specify one |

---

## [brain.workflow] — Workflow Engine Configuration

| Key | Type | Default | Description |
|---|---|---|---|
| `checkpoint_interval_secs` | `u64` | `60` | Seconds between automatic checkpoints |
| `checkpoint_max_size_bytes` | `u32` | `1048576` | Discard checkpoint if serialised state exceeds this (1 MB) |
| `max_checkpoints_per_workflow` | `usize` | `50` | Ring buffer; oldest checkpoint is evicted |
| `step_timeout_ms` | `u64` | `300000` | Per-step wall-clock timeout (5 minutes) |
| `max_retries_per_step` | `u32` | `3` | Retry failed steps before triggering workflow failure |
| `fan_out_max_branches` | `usize` | `20` | Hard limit on FanOut concurrency |
| `recovery_strategy` | `string` | `"retry_then_checkpoint"` | `retry_then_checkpoint` / `skip_and_continue` / `fail_fast` |

---

## [brain.learning] — Learning Configuration

| Key | Type | Default | Description |
|---|---|---|---|
| `learning_cycle_interval_secs` | `u64` | `300` | Periodic full learning cycle (5 minutes) |
| `promotion_threshold` | `f64` | `0.8` | Minimum score for episodic → semantic promotion |
| `max_promotions_per_cycle` | `usize` | `100` | Cap to prevent thundering-herd writes |
| `memory_access_recency_weight` | `f64` | `0.5` | Weight given to recency in `should_promote()` |
| `memory_access_frequency_weight` | `f64` | `0.3` | Weight given to access count |
| `memory_importance_weight` | `f64` | `0.2` | Weight given to operator-assigned importance |
| `planner_adjustment_enabled` | `bool` | `true` | Feed lessons back into planner strategy weights |
| `reasoner_heuristic_update_enabled` | `bool` | `true` | Feed lessons back into reasoner heuristics |

---

## [brain.model] — Model Provider Configuration

Abstracted behind `brain-model` traits. Specifies which providers are available and their capabilities; does not configure provider-specific parameters (those live in the provider's own config).

| Key | Type | Default | Description |
|---|---|---|---|
| `default_reasoning_provider` | `string` | `""` | Provider name for reasoning calls |
| `default_planning_provider` | `string` | `""` | Provider name for planning calls |
| `fallback_provider` | `string` | `""` | Used when the primary is unavailable |
| `provider_timeout_ms` | `u64` | `30000` | Default HTTP timeout for model calls |
| `max_concurrent_model_calls` | `usize` | `4` | Tokio semaphore limit |
| `circuit_breaker_failure_threshold` | `u32` | `5` | Failures before a provider is marked unavailable |
| `circuit_breaker_recovery_secs` | `u64` | `60` | Cooldown before retrying an unavailable provider |

---

## [brain.diagnostics] — Observability Configuration

| Key | Type | Default | Description |
|---|---|---|---|
| `trace_level` | `string` | `"standard"` | `minimal` / `standard` / `verbose` |
| `retain_event_history_secs` | `u64` | `3600` | In-memory event window for replay |
| `budget_report_interval_secs` | `u64` | `30` | Emit budget utilisation summary |
| `state_transition_log_level` | `string` | `"debug"` | `trace` / `debug` / `info` / `warn` |
| `slow_cycle_threshold_ms` | `u64` | `1000` | Warn if a single tick exceeds this |

---

## Example: Full brain.toml

```toml
[brain]
max_active_goals = 3
goal_scheduling = "priority"
goal_monitoring_interval_secs = 5
loop_tick_timeout_ms = 5000

[brain.budget]
default_max_thinking_time_ms = 30000
default_max_iterations = 10
default_max_reasoning_depth = 5
default_max_branches = 3
default_max_model_calls = 5
default_max_token_budget = 8192
default_max_cost_budget_cents = 50

[brain.reasoning]
max_plan_steps = 50
retry_limit = 3
replan_strategy_order = ["means_ends", "forward_chaining", "backward_chaining", "template_match"]
hypothesis_confidence_threshold = 0.6

[brain.decision]
default_strategy = "utility_maximize"
confidence_threshold = 0.7
conflict_resolution_policy = "escalate"

[brain.decision.utility_weights]
cost = 0.3
benefit = 0.4
risk = 0.2
speed = 0.05
resource = 0.05

[brain.policy]
rulesets_path = "configs/policy/rulesets/"
guard_rails_path = "configs/policy/guard_rails.json"
rule_reload_interval_secs = 60
evaluation_order = "prioritized"

[brain.goals]
persistence_interval_secs = 30
auto_cancel_on_parent_fail = true

[brain.workflow]
checkpoint_interval_secs = 60
max_retries_per_step = 3
fan_out_max_branches = 20

[brain.learning]
learning_cycle_interval_secs = 300
promotion_threshold = 0.8
max_promotions_per_cycle = 100

[brain.model]
provider_timeout_ms = 30000
max_concurrent_model_calls = 4
circuit_breaker_failure_threshold = 5

[brain.diagnostics]
trace_level = "standard"
budget_report_interval_secs = 30
```

---

## Cross-References

- [`docs/architecture/brain.md`](brain.md) — subsystem responsibilities and interfaces
- [`docs/architecture/brain-dependencies.md`](brain-dependencies.md) — dependency rules
- [`docs/interfaces/brain-events.md`](brain-events.md) — events emitted/consumed by each subsystem
- [`cognitive-loop.md`](cognitive-loop.md) — cognitive cycle, state machines, and flows
