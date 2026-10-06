//! AWEF-owned browser/navigation kernel.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationEntry {
    pub id: u64,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationError {
    EmptyTarget,
}

#[derive(Debug, Clone, Default)]
pub struct BrowserKernel {
    history: Vec<NavigationEntry>,
    current: Option<usize>,
    next_id: u64,
}

impl BrowserKernel {
    pub fn navigate(
        &mut self,
        target: impl Into<String>,
    ) -> Result<&NavigationEntry, NavigationError> {
        let target = target.into();
        if target.trim().is_empty() {
            return Err(NavigationError::EmptyTarget);
        }

        if let Some(current) = self.current {
            self.history.truncate(current + 1);
        }

        self.next_id = self.next_id.saturating_add(1);
        self.history.push(NavigationEntry {
            id: self.next_id,
            target,
        });
        self.current = Some(self.history.len() - 1);
        Ok(self.current().unwrap())
    }

    pub fn current(&self) -> Option<&NavigationEntry> {
        self.current.and_then(|index| self.history.get(index))
    }

    pub fn back(&mut self) -> Option<&NavigationEntry> {
        let current = self.current?;
        if current == 0 {
            return self.history.get(current);
        }
        self.current = Some(current - 1);
        self.current()
    }

    pub fn forward(&mut self) -> Option<&NavigationEntry> {
        let current = self.current?;
        if current + 1 >= self.history.len() {
            return self.history.get(current);
        }
        self.current = Some(current + 1);
        self.current()
    }

    pub fn history(&self) -> &[NavigationEntry] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_truncates_forward_history() {
        let mut kernel = BrowserKernel::default();
        kernel.navigate("awef://one").unwrap();
        kernel.navigate("awef://two").unwrap();
        kernel.back();
        kernel.navigate("awef://three").unwrap();

        assert_eq!(kernel.history().len(), 2);
        assert_eq!(kernel.current().unwrap().target, "awef://three");
    }

    #[test]
    fn empty_navigation_fails_closed() {
        let mut kernel = BrowserKernel::default();
        assert_eq!(kernel.navigate("   "), Err(NavigationError::EmptyTarget));
    }
}
