# Reflection Engine Architecture

## Overview

The Reflection Engine analyzes completed goals to extract lessons learned, identify patterns, and improve future execution. It operates in the `brain-reflection` crate and integrates with the `brain-learning` crate for pattern consolidation.

## Implementation

### brain-reflection

| Component | File | Responsibility |
|---|---|---|
| `Reflector` | `reflector.rs` | Core reflection logic |
| `LessonStore` | `lesson_store.rs` | Persists lessons learned |
| `Lesson` | `types.rs` | Structured lesson representation |

### brain-learning

| Component | File | Responsibility |
|---|---|---|
| `LearningEngine` | `learning_engine.rs` | Consolidates lessons into knowledge |
| `KnowledgeBase` | `knowledge_base.rs` | Stores patterns and workflows |
| `PatternRecognizer` | `pattern_recognizer.rs` | Identifies recurring patterns |

## Reflection Lifecycle

```
Goal Completed
    │
    ▼
Reflector.reflect(goal_id, expected, actual)
    │
    ▼
Lesson {
    goal_id,
    what_worked: Vec<String>,
    what_failed: Vec<String>,
    retries: u32,
    bottleneck: Option<String>,
    execution_time_ms: u64,
    capability_usage: Vec<CapabilityId>,
    lesson_text: String,
}
    │
    ▼
LearningEngine.learn_from_lesson(lesson)
    │
    ▼
ConsolidationReport {
    patterns_identified: Vec<Pattern>,
    strategy_adjustments: Vec<StrategyAdjustment>,
    cognitive_health: f32,
}
    │
    ▼
Store in memory-semantic KnowledgeBase
    │
    ▼
Apply strategy adjustments to StrategyEngine
```

## Reflection Input

The `Reflector` receives:
- **goal_id** — which goal was completed
- **expected_outcome** — what was supposed to happen
- **actual_outcome** — what actually happened
- **execution metrics** — retries, time, capabilities used

## Reflection Output

### Lesson
```rust
pub struct Lesson {
    pub lesson_id: LessonId,
    pub goal_id: GoalId,
    pub timestamp: Timestamp,
    pub what_worked: Vec<String>,
    pub what_failed: Vec<String>,
    pub retries: u32,
    pub bottleneck: Option<String>,
    pub execution_time_ms: u64,
    pub capability_usage: Vec<CapabilityId>,
    pub lesson_text: String,
}
```

### ConsolidationReport
```rust
pub struct ConsolidationReport {
    pub report_id: ReportId,
    pub lesson_id: LessonId,
    pub patterns_identified: Vec<Pattern>,
    pub strategy_adjustments: Vec<StrategyAdjustment>,
    pub cognitive_health: f32,  // 0.0–1.0
}
```

## Pattern Recognition

The `PatternRecognizer` identifies recurring patterns across lessons:
- **Repeated bugs** — same capability fails across multiple goals
- **Successful workflows** — same sequence succeeds consistently
- **Bottleneck patterns** — specific capabilities consistently slow execution
- **Retry patterns** — certain failure types benefit from specific retry strategies

## Strategy Adjustment

After learning, the `StrategyEngine` adjusts execution strategies:

| Adjustment | Effect |
|---|---|
| `direct` → `sequential` | Switch from parallel to sequential when dependencies detected |
| `sequential` → `parallel` | Switch to parallel when no dependencies found |
| `delegated` → `direct` | Take over execution when agent unavailable |
| Add retry policy | Increase retries for flaky capabilities |
| Add timeout | Reduce timeout for slow capabilities |

## Integration with Cognitive Loop

The reflection engine is called in `BrainOrchestrator`:

```rust
// After goal completion:
let reflection = self.reflect_on_workflow(&expected, &actual)?;
let report = self.learn_from_reflection(&reflection.lesson)?;
self.apply_learning_feedback(&report.feedback);
```

And in `CognitiveLoopService`:
```rust
// After each cycle:
if decision.is_terminal() {
    let lesson = self.reflector.reflect(goal_id, expected, actual);
    self.learning_engine.learn_from_lesson(lesson);
}
```

## Journal Output

Reflection results are stored in `docs/journal/` as structured entries:

```markdown
## Reflection Entry

**Goal**: Fix bootstrap crash
**Outcome**: Success (2 retries)
**Bottleneck**: Capability discovery took 3s
**Lesson**: Pre-warm capability registry at startup
**Strategy Adjustment**: Increase initial retry count for discovery
```

## Testing

- `brain/brain-reflection/src/tests.rs` — unit tests for reflector and lesson store
- `brain/brain-learning/src/tests.rs` — unit tests for pattern recognition
- `tests/cognitive-integration/tests/pipeline.rs` — integration tests verifying reflection completes after goal execution

## Future Enhancements

1. **Cross-goal pattern analysis** — identify patterns across multiple goals
2. **Capability performance profiling** — track per-capability success rates over time
3. **Automated strategy tuning** — adjust retry/timeout parameters based on history
4. **Semantic retrieval** — query past lessons when facing similar goals