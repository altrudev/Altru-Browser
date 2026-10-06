//! AWEF-owned JavaScript host contract.
//!
//! Implementations such as Boa may be evaluated behind this trait. No script
//! implementation receives ambient network/filesystem authority.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptBudget {
    pub instruction_budget: u64,
    pub wall_clock_budget_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptOutcome {
    Completed,
    Yielded,
    BudgetExceeded,
    Rejected(String),
}

pub trait ScriptHost {
    fn queue_microtask(&mut self, token: u64);
    fn record_dom_mutation(&mut self, node: usize);
}

pub trait ScriptEngine {
    fn name(&self) -> &'static str;
    fn execute(
        &mut self,
        source: &str,
        budget: ScriptBudget,
        host: &mut dyn ScriptHost,
    ) -> ScriptOutcome;
}

#[derive(Debug, Default)]
pub struct DisabledScriptEngine;

impl ScriptEngine for DisabledScriptEngine {
    fn name(&self) -> &'static str {
        "disabled"
    }

    fn execute(
        &mut self,
        _source: &str,
        _budget: ScriptBudget,
        _host: &mut dyn ScriptHost,
    ) -> ScriptOutcome {
        ScriptOutcome::Rejected("script engine not promoted".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Host;

    impl ScriptHost for Host {
        fn queue_microtask(&mut self, _token: u64) {}
        fn record_dom_mutation(&mut self, _node: usize) {}
    }

    #[test]
    fn scripts_fail_closed_before_engine_promotion() {
        let mut engine = DisabledScriptEngine;
        let mut host = Host;
        assert!(matches!(
            engine.execute(
                "1 + 1",
                ScriptBudget {
                    instruction_budget: 100,
                    wall_clock_budget_ms: 10,
                },
                &mut host
            ),
            ScriptOutcome::Rejected(_)
        ));
    }
}
