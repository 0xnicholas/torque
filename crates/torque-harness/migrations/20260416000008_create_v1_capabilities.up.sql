-- up
CREATE TABLE v1_capability_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    input_contract JSONB,
    output_contract JSONB,
    risk_level TEXT NOT NULL DEFAULT 'low',
    default_agent_definition_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE v1_capability_registry_bindings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    capability_profile_id UUID NOT NULL REFERENCES v1_capability_profiles(id) ON DELETE CASCADE,
    agent_definition_id UUID NOT NULL REFERENCES v1_agent_definitions(id) ON DELETE CASCADE,
    compatibility_score DOUBLE PRECISION,
    quality_tier TEXT NOT NULL DEFAULT 'experimental',
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
