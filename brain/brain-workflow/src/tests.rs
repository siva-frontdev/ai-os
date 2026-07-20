#[cfg(test)]
mod tests {
    use brain_core::ids::{GoalId, PlanId, ToolCapabilityId};
    use brain_core::tool::{ExecutablePlan, ToolRequirement, ToolValue};
    use std::collections::HashMap;
    use crate::types::WorkflowStatus;
    use crate::workflow_builder::WorkflowBuilder;
    use crate::workflow_executor::WorkflowExecutor;

    fn make_plan(_name: &str, step_count: usize) -> ExecutablePlan {
        let reqs: Vec<ToolRequirement> = (0..step_count)
            .map(|i| {
                let mut inputs = HashMap::new();
                inputs.insert("idx".into(), ToolValue::Number(i as f64));
                ToolRequirement::new(
                    ToolCapabilityId::new(),
                    inputs,
                    vec!["output".into()],
                )
            })
            .collect();

        ExecutablePlan::new(
            PlanId::new(),
            GoalId::new(),
            reqs,
        )
    }

    #[test]
    fn test_workflow_builder_creates_workflow() {
        let plan = make_plan("test-plan", 3);
        let builder = WorkflowBuilder::new();
        let wf = builder.build("test-workflow", plan, 2);
        assert_eq!(wf.name, "test-workflow");
        assert_eq!(wf.steps.len(), 3);
        assert_eq!(wf.status, WorkflowStatus::Pending);
    }

    #[test]
    fn test_workflow_executor_lifecycle() {
        let exec = WorkflowExecutor::new();
        let plan = make_plan("test", 2);
        let id = exec.create_workflow("lifecycle-test", plan, 2).unwrap();
        exec.start(&id).unwrap();

        let wf = exec.get_workflow(&id).unwrap();
        assert_eq!(wf.status, WorkflowStatus::Running);

        exec.complete_step(&id, 0, "step-0 done".into()).unwrap();
        exec.complete_step(&id, 1, "step-1 done".into()).unwrap();

        let wf = exec.get_workflow(&id).unwrap();
        assert_eq!(wf.status, WorkflowStatus::Completed);
    }

    #[test]
    fn test_workflow_executor_retry_then_fail() {
        let exec = WorkflowExecutor::new();
        let plan = make_plan("retry-test", 1);
        let id = exec.create_workflow("retry-test", plan, 2).unwrap();
        exec.start(&id).unwrap();

        exec.fail_step(&id, 0, "first fail".into()).unwrap();
        let wf = exec.get_workflow(&id).unwrap();
        assert_eq!(wf.status, WorkflowStatus::Running);

        exec.fail_step(&id, 0, "second fail".into()).unwrap();
        let wf = exec.get_workflow(&id).unwrap();
        assert_eq!(wf.status, WorkflowStatus::Failed);
    }

    #[test]
    fn test_workflow_pause_resume() {
        let exec = WorkflowExecutor::new();
        let plan = make_plan("pause-test", 2);
        let id = exec.create_workflow("pause-test", plan, 1).unwrap();
        exec.start(&id).unwrap();

        exec.pause(&id).unwrap();
        let wf = exec.get_workflow(&id).unwrap();
        assert_eq!(wf.status, WorkflowStatus::Paused);

        exec.resume(&id).unwrap();
        let wf = exec.get_workflow(&id).unwrap();
        assert_eq!(wf.status, WorkflowStatus::Running);
    }

    #[test]
    fn test_workflow_cancel() {
        let exec = WorkflowExecutor::new();
        let plan = make_plan("cancel-test", 2);
        let id = exec.create_workflow("cancel-test", plan, 1).unwrap();
        exec.start(&id).unwrap();
        exec.cancel(&id).unwrap();
        let wf = exec.get_workflow(&id).unwrap();
        assert_eq!(wf.status, WorkflowStatus::Cancelled);
    }

    #[test]
    fn test_workflow_progress() {
        let exec = WorkflowExecutor::new();
        let plan = make_plan("progress", 3);
        let id = exec.create_workflow("progress", plan, 1).unwrap();
        exec.start(&id).unwrap();

        let prog = exec.get_progress(&id).unwrap();
        assert_eq!(prog.total_steps, 3);

        exec.complete_step(&id, 0, "done".into()).unwrap();
        exec.complete_step(&id, 1, "done".into()).unwrap();

        let prog = exec.get_progress(&id).unwrap();
        assert_eq!(prog.completed_steps, 2);
        assert!((prog.progress_pct - 2.0/3.0).abs() < 0.01);
    }

    #[test]
    fn test_list_workflows() {
        let exec = WorkflowExecutor::new();
        let p1 = make_plan("plan-1", 1);
        let p2 = make_plan("plan-2", 1);
        exec.create_workflow("wf-1", p1, 1).unwrap();
        exec.create_workflow("wf-2", p2, 1).unwrap();
        let list = exec.list_workflows().unwrap();
        assert_eq!(list.len(), 2);
    }
}
