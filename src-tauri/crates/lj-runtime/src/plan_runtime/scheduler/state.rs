//! execution-local port values 与 invocation ordinal 分配。

use super::{BTreeMap, FlowPortRef, InvocationPath, LoopInvocationSegment, RuntimeValue, Uuid};

#[derive(Default)]
pub(in crate::plan_runtime::scheduler) struct PortState {
    inputs: BTreeMap<FlowPortRef, RuntimeValue>,
    outputs: BTreeMap<FlowPortRef, RuntimeValue>,
}

impl PortState {
    pub(in crate::plan_runtime::scheduler) fn seed_input(
        &mut self,
        port: FlowPortRef,
        value: RuntimeValue,
    ) {
        self.inputs.insert(port, value);
    }

    pub(in crate::plan_runtime::scheduler) fn input(
        &self,
        node_id: Uuid,
        handle: &str,
    ) -> Option<RuntimeValue> {
        self.inputs.get(&FlowPortRef::new(node_id, handle)).cloned()
    }

    pub(in crate::plan_runtime::scheduler) fn output(
        &self,
        port: &FlowPortRef,
    ) -> Option<RuntimeValue> {
        self.outputs.get(port).cloned()
    }

    pub(in crate::plan_runtime::scheduler) fn route(
        &mut self,
        plan: &lj_rule_model::ExecutionPlan,
        node_id: Uuid,
        handle: &str,
        value: &RuntimeValue,
    ) -> Result<(), &'static str> {
        let source = FlowPortRef::new(node_id, handle);
        self.outputs.insert(source.clone(), (*value).clone());
        for edge in plan.edges().iter().filter(|edge| edge.from == source) {
            if self
                .inputs
                .insert(edge.to.clone(), (*value).clone())
                .is_some()
            {
                return Err("同一 input 收到多个 active producer");
            }
        }
        Ok(())
    }
}

pub(in crate::plan_runtime::scheduler) struct InvocationSequence {
    next_ordinal: u64,
}

impl InvocationSequence {
    pub(in crate::plan_runtime::scheduler) const fn new() -> Self {
        Self { next_ordinal: 1 }
    }

    pub(in crate::plan_runtime::scheduler) fn next(
        &mut self,
        node_id: Uuid,
        loop_iterations: &[LoopInvocationSegment],
    ) -> Result<InvocationPath, ()> {
        let ordinal = self.next_ordinal;
        self.next_ordinal = ordinal.checked_add(1).ok_or(())?;
        InvocationPath::new(node_id, loop_iterations.to_vec(), ordinal).map_err(|_| ())
    }

    pub(in crate::plan_runtime::scheduler) const fn observed(&self) -> u64 {
        self.next_ordinal - 1
    }
}
