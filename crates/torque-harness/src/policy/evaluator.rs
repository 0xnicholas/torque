use super::{PolicyDecision, PolicyInput, PolicySources};

pub struct PolicyEvaluator;

impl PolicyEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Evaluate policy across multiple sources with dimensional conservative merge.
    ///
    /// All 6 dimensions are evaluated independently across all applicable sources.
    /// Within each dimension, the most restrictive rule wins.
    /// Dimensions are isolated: a tool deny does not affect memory, etc.
    pub fn evaluate(&self, input: &PolicyInput, sources: &PolicySources) -> PolicyDecision {
        let decisions = [
            self.evaluate_tool(input.tool_name.as_deref(), sources),
            self.evaluate_approval(sources),
            self.evaluate_visibility(sources),
            self.evaluate_delegation(sources),
            self.evaluate_resource(sources),
            self.evaluate_memory(sources),
        ];

        decisions
            .into_iter()
            .fold(PolicyDecision::default(), |acc, d| acc.merge(d))
    }

    // ── Tool dimension ──────────────────────────────────────────────────

    fn evaluate_tool(&self, tool_name: Option<&str>, sources: &PolicySources) -> PolicyDecision {
        let source_order: [(&str, &Option<serde_json::Value>); 6] = [
            ("system", &sources.system),
            ("capability", &sources.capability),
            ("agent", &sources.agent),
            ("team", &sources.team),
            ("selector", &sources.selector),
            ("runtime", &sources.runtime),
        ];

        source_order
            .iter()
            .fold(PolicyDecision::default(), |acc, (source_name, source)| {
                if let Some(policy) = source {
                    acc.merge(Self::evaluate_single_source_tool(
                        tool_name,
                        policy,
                        source_name,
                    ))
                } else {
                    acc
                }
            })
    }

    fn evaluate_single_source_tool(
        tool_name: Option<&str>,
        policy: &serde_json::Value,
        source_name: &str,
    ) -> PolicyDecision {
        if policy.is_null() || policy.as_object().map_or(true, |o| o.is_empty()) {
            return PolicyDecision::default();
        }

        // If a specific tool name is provided, evaluate against it.
        // Otherwise evaluate all rules in the source and take the most restrictive.
        let name = match tool_name {
            Some(n) => n,
            None => {
                // No specific tool — if this source forbids anything,
                // treat as a blanket restriction
                if let Some(forbidden) = policy.get("forbidden_tools").and_then(|v| v.as_array()) {
                    for forbidden_tool in forbidden {
                        if let Some(name) = forbidden_tool.as_str() {
                            if name == "*" {
                                return PolicyDecision::deny("All tools forbidden by policy");
                            }
                            // Add to restrictions only — allowed stays true since
                            // we don't know if this specific tool is being called
                        }
                    }
                }
                return PolicyDecision::default();
            }
        };

        // Check forbidden tools
        if let Some(forbidden) = policy.get("forbidden_tools").and_then(|v| v.as_array()) {
            for forbidden_tool in forbidden {
                if let Some(forbidden_name) = forbidden_tool.as_str() {
                    if forbidden_name == name || forbidden_name == "*" {
                        return PolicyDecision::deny(format!(
                            "Tool '{}' is forbidden by {} policy",
                            name, source_name
                        ));
                    }
                }
            }
        }

        // Check require_approval tools
        if let Some(require_approval) = policy
            .get("require_approval_tools")
            .and_then(|v| v.as_array())
        {
            for approval_tool in require_approval {
                if let Some(approval_name) = approval_tool.as_str() {
                    if approval_name == name || approval_name == "*" {
                        return PolicyDecision::require_approval(
                            "tool",
                            format!(
                                "Tool '{}' requires approval by {} policy",
                                name, source_name
                            ),
                        );
                    }
                }
            }
        }

        // Check allowed tools (whitelist)
        if let Some(allowed) = policy.get("allowed_tools").and_then(|v| v.as_array()) {
            let mut is_allowed = false;
            for allowed_tool in allowed {
                if let Some(allowed_name) = allowed_tool.as_str() {
                    if allowed_name == name || allowed_name == "*" {
                        is_allowed = true;
                        break;
                    }
                }
            }
            if !is_allowed {
                return PolicyDecision::deny(format!(
                    "Tool '{}' is not in {} allowed tools list",
                    name, source_name
                ));
            }
        }

        PolicyDecision::default()
    }

    // ── Approval dimension ──────────────────────────────────────────────

    fn evaluate_approval(&self, sources: &PolicySources) -> PolicyDecision {
        self.evaluate_dimension(sources, Self::evaluate_single_source_approval)
    }

    fn evaluate_single_source_approval(policy: &serde_json::Value) -> PolicyDecision {
        if policy.is_null() || policy.as_object().map_or(true, |o| o.is_empty()) {
            return PolicyDecision::default();
        }

        let mut decision = PolicyDecision::default();

        if let Some(required) = policy.get("approval_required").and_then(|v| v.as_bool()) {
            if required {
                decision.requires_approval = true;
                decision.reasons.push("Approval required by policy".into());
            }
        }

        if let Some(requirements) = policy.get("approval_requirements").and_then(|v| v.as_array()) {
            for req in requirements {
                if let Some(dim) = req.as_str() {
                    decision.approval_dimensions.push(dim.to_string());
                }
            }
        }

        if let Some(escalation) = policy
            .get("require_operator_escalation")
            .and_then(|v| v.as_bool())
        {
            if escalation {
                decision.approval_dimensions.push("operator_escalation".into());
            }
        }

        decision
    }

    // ── Visibility dimension ────────────────────────────────────────────

    fn evaluate_visibility(&self, sources: &PolicySources) -> PolicyDecision {
        self.evaluate_dimension(sources, Self::evaluate_single_source_visibility)
    }

    fn evaluate_single_source_visibility(policy: &serde_json::Value) -> PolicyDecision {
        if policy.is_null() || policy.as_object().map_or(true, |o| o.is_empty()) {
            return PolicyDecision::default();
        }

        // Narrowing signal from visibility_scope
        if let Some(scope) = policy.get("visibility_scope").and_then(|v| v.as_str()) {
            if scope == "narrow" {
                return PolicyDecision {
                    visibility_restriction: Some("narrow".into()),
                    reasons: vec!["Visibility narrowed by policy".into()],
                    ..PolicyDecision::default()
                };
            }
        }

        // Denied scopes
        if let Some(denied) = policy.get("denied_scopes").and_then(|v| v.as_array()) {
            let denied_list: Vec<String> = denied
                .iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect();
            if !denied_list.is_empty() {
                let reason = format!("Scopes denied: {}", denied_list.join(", "));
                return PolicyDecision {
                    visibility_restriction: Some(format!("denied:{}", denied_list.join(","))),
                    reasons: vec![reason],
                    ..PolicyDecision::default()
                };
            }
        }

        PolicyDecision::default()
    }

    // ── Delegation dimension ────────────────────────────────────────────

    fn evaluate_delegation(&self, sources: &PolicySources) -> PolicyDecision {
        self.evaluate_dimension(sources, Self::evaluate_single_source_delegation)
    }

    fn evaluate_single_source_delegation(policy: &serde_json::Value) -> PolicyDecision {
        if policy.is_null() || policy.as_object().map_or(true, |o| o.is_empty()) {
            return PolicyDecision::default();
        }

        let mut restrictions: Vec<String> = Vec::new();

        if let Some(allowed) = policy.get("delegation_allowed").and_then(|v| v.as_bool()) {
            if !allowed {
                restrictions.push("Delegation denied by policy".into());
            }
        }

        if let Some(max_depth) = policy.get("max_delegation_depth").and_then(|v| v.as_i64()) {
            restrictions.push(format!("max_delegation_depth={}", max_depth));
        }

        if let Some(child) = policy
            .get("child_delegation_allowed")
            .and_then(|v| v.as_bool())
        {
            if !child {
                restrictions.push("child_delegation_disallowed".into());
            }
        }

        if let Some(handoff) = policy.get("handoff_allowed").and_then(|v| v.as_bool()) {
            if !handoff {
                restrictions.push("handoff_disallowed".into());
            }
        }

        if restrictions.is_empty() {
            PolicyDecision::default()
        } else {
            PolicyDecision {
                delegation_restrictions: restrictions.clone(),
                reasons: restrictions,
                ..PolicyDecision::default()
            }
        }
    }

    // ── Resource dimension ──────────────────────────────────────────────

    fn evaluate_resource(&self, sources: &PolicySources) -> PolicyDecision {
        self.evaluate_dimension(sources, Self::evaluate_single_source_resource)
    }

    fn evaluate_single_source_resource(policy: &serde_json::Value) -> PolicyDecision {
        if policy.is_null() || policy.as_object().map_or(true, |o| o.is_empty()) {
            return PolicyDecision::default();
        }

        let mut limits: Vec<String> = Vec::new();

        if let Some(cap) = policy.get("resource_budget_cap").and_then(|v| v.as_i64()) {
            limits.push(format!("budget_cap={}", cap));
        }

        if let Some(concurrency) = policy.get("max_concurrency").and_then(|v| v.as_i64()) {
            limits.push(format!("max_concurrency={}", concurrency));
        }

        if let Some(timeout) = policy.get("timeout_seconds").and_then(|v| v.as_i64()) {
            limits.push(format!("timeout={}", timeout));
        }

        if let Some(defer) = policy
            .get("defer_under_pressure")
            .and_then(|v| v.as_bool())
        {
            if defer {
                limits.push("defer_under_pressure".into());
            }
        }

        if limits.is_empty() {
            PolicyDecision::default()
        } else {
            PolicyDecision {
                resource_limits: limits.clone(),
                reasons: limits,
                ..PolicyDecision::default()
            }
        }
    }

    // ── Memory dimension ────────────────────────────────────────────────

    fn evaluate_memory(&self, sources: &PolicySources) -> PolicyDecision {
        self.evaluate_dimension(sources, Self::evaluate_single_source_memory)
    }

    fn evaluate_single_source_memory(policy: &serde_json::Value) -> PolicyDecision {
        if policy.is_null() || policy.as_object().map_or(true, |o| o.is_empty()) {
            return PolicyDecision::default();
        }

        let mut restrictions: Vec<String> = Vec::new();

        if let Some(write_allowed) = policy
            .get("memory_write_allowed")
            .and_then(|v| v.as_bool())
        {
            if !write_allowed {
                restrictions.push("memory_write_denied".into());
            }
        }

        if let Some(candidate_only) = policy
            .get("memory_candidate_only")
            .and_then(|v| v.as_bool())
        {
            if candidate_only {
                restrictions.push("memory_candidate_only".into());
            }
        }

        if let Some(max_entries) = policy.get("max_memory_entries").and_then(|v| v.as_i64()) {
            restrictions.push(format!("max_entries={}", max_entries));
        }

        if let Some(review) = policy
            .get("require_review_before_write")
            .and_then(|v| v.as_bool())
        {
            if review {
                restrictions.push("require_review_before_write".into());
            }
        }

        if restrictions.is_empty() {
            PolicyDecision::default()
        } else {
            PolicyDecision {
                memory_restrictions: restrictions.clone(),
                reasons: restrictions,
                ..PolicyDecision::default()
            }
        }
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    fn evaluate_dimension<F>(&self, sources: &PolicySources, eval_single: F) -> PolicyDecision
    where
        F: Fn(&serde_json::Value) -> PolicyDecision,
    {
        let source_order: [(&str, &Option<serde_json::Value>); 6] = [
            ("system", &sources.system),
            ("capability", &sources.capability),
            ("agent", &sources.agent),
            ("team", &sources.team),
            ("selector", &sources.selector),
            ("runtime", &sources.runtime),
        ];

        source_order
            .iter()
            .fold(PolicyDecision::default(), |acc, (_name, source)| {
                if let Some(policy) = source {
                    acc.merge(eval_single(policy))
                } else {
                    acc
                }
            })
    }
}

impl Default for PolicyEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
