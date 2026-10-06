use crate::model::{LifecycleState, ResourceState};

pub fn lifecycle_for(state: ResourceState) -> LifecycleState {
    if state.visible || state.recently_interacted {
        return LifecycleState::Hot;
    }

    if state.memory_available_ratio < 0.10 {
        return LifecycleState::Frozen;
    }

    if state.memory_available_ratio < 0.20 || state.cpu_busy_ratio > 0.85 {
        return LifecycleState::Cold;
    }

    LifecycleState::Warm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_content_stays_hot_even_under_pressure() {
        let state = lifecycle_for(ResourceState {
            memory_available_ratio: 0.05,
            cpu_busy_ratio: 0.99,
            visible: true,
            recently_interacted: false,
        });
        assert_eq!(state, LifecycleState::Hot);
    }

    #[test]
    fn invisible_content_freezes_at_critical_memory() {
        let state = lifecycle_for(ResourceState {
            memory_available_ratio: 0.08,
            cpu_busy_ratio: 0.30,
            visible: false,
            recently_interacted: false,
        });
        assert_eq!(state, LifecycleState::Frozen);
    }

    #[test]
    fn cpu_pressure_cools_background_work() {
        let state = lifecycle_for(ResourceState {
            memory_available_ratio: 0.60,
            cpu_busy_ratio: 0.91,
            visible: false,
            recently_interacted: false,
        });
        assert_eq!(state, LifecycleState::Cold);
    }
}
