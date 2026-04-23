use uuid::Uuid;

pub struct StreamKeys;

impl StreamKeys {
    pub fn team_shared_pool(team_id: Uuid) -> String {
        format!("team:{}:tasks:shared", team_id)
    }

    pub fn member_tasks(member_id: Uuid) -> String {
        format!("member:{}:tasks", member_id)
    }

    pub fn delegation_status(delegation_id: Uuid) -> String {
        format!("delegation:{}:status", delegation_id)
    }
}

pub const TEAM_SHARED_POOL: StreamKeys = StreamKeys;
pub const MEMBER_TASKS: StreamKeys = StreamKeys;
pub const DELEGATION_STATUS: StreamKeys = StreamKeys;
