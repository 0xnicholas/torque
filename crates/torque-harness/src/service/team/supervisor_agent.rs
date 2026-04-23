pub struct SupervisorAgent {
    _private: (),
}

impl SupervisorAgent {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for SupervisorAgent {
    fn default() -> Self {
        Self::new()
    }
}