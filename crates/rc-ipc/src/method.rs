//! The typed list of IPC methods. The server dispatch matches on [`Method`],
//! and the generated reference reads [`specs`], so both follow this list.
use std::path::PathBuf;

use schemars::{Schema, SchemaGenerator};

use crate::schema::{CfgParams, GetSettingsResult, IdParams, PingResult, SetSettingsParams};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Ping,
    Shutdown,
    ListInstances,
    AddInstance,
    UpdateInstance,
    RemoveInstance,
    StartInstance,
    StopInstance,
    RestartInstance,
    GetLogs,
    StartAll,
    StopAll,
    GetSettings,
    SetSettings,
    DetectClaudeBin,
}

pub const ALL: [Method; 15] = [
    Method::Ping,
    Method::Shutdown,
    Method::ListInstances,
    Method::AddInstance,
    Method::UpdateInstance,
    Method::RemoveInstance,
    Method::StartInstance,
    Method::StopInstance,
    Method::RestartInstance,
    Method::GetLogs,
    Method::StartAll,
    Method::StopAll,
    Method::GetSettings,
    Method::SetSettings,
    Method::DetectClaudeBin,
];

impl Method {
    /// Wire name, as sent in the `method` field.
    pub fn name(self) -> &'static str {
        match self {
            Method::Ping => "ping",
            Method::Shutdown => "shutdown",
            Method::ListInstances => "list_instances",
            Method::AddInstance => "add_instance",
            Method::UpdateInstance => "update_instance",
            Method::RemoveInstance => "remove_instance",
            Method::StartInstance => "start_instance",
            Method::StopInstance => "stop_instance",
            Method::RestartInstance => "restart_instance",
            Method::GetLogs => "get_logs",
            Method::StartAll => "start_all",
            Method::StopAll => "stop_all",
            Method::GetSettings => "get_settings",
            Method::SetSettings => "set_settings",
            Method::DetectClaudeBin => "detect_claude_bin",
        }
    }

    pub fn from_name(name: &str) -> Option<Method> {
        ALL.into_iter().find(|m| m.name() == name)
    }
}

/// One method's params and result, as JSON Schema.
pub struct MethodSpec {
    pub method: Method,
    /// `None` when the method takes no params.
    pub params: Option<Schema>,
    pub result: Schema,
}

/// Schemas for every method, in [`ALL`] order. Types come from `generator`.
pub fn specs(generator: &mut SchemaGenerator) -> Vec<MethodSpec> {
    ALL.into_iter().map(|m| spec(m, generator)).collect()
}

fn spec(method: Method, g: &mut SchemaGenerator) -> MethodSpec {
    use crate::schema::InstanceView;
    use Method::*;
    // A `()` result is JSON `null`.
    let (params, result) = match method {
        Ping => (None, g.subschema_for::<PingResult>()),
        Shutdown => (None, g.subschema_for::<()>()),
        ListInstances => (None, g.subschema_for::<Vec<InstanceView>>()),
        AddInstance => (
            Some(g.subschema_for::<CfgParams>()),
            g.subschema_for::<String>(),
        ),
        UpdateInstance => (
            Some(g.subschema_for::<CfgParams>()),
            g.subschema_for::<()>(),
        ),
        RemoveInstance | StartInstance | StopInstance | RestartInstance => {
            (Some(g.subschema_for::<IdParams>()), g.subschema_for::<()>())
        }
        GetLogs => (
            Some(g.subschema_for::<IdParams>()),
            g.subschema_for::<Vec<String>>(),
        ),
        StartAll | StopAll => (None, g.subschema_for::<()>()),
        GetSettings => (None, g.subschema_for::<GetSettingsResult>()),
        SetSettings => (
            Some(g.subschema_for::<SetSettingsParams>()),
            g.subschema_for::<()>(),
        ),
        DetectClaudeBin => (None, g.subschema_for::<Option<PathBuf>>()),
    };
    MethodSpec {
        method,
        params,
        result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_are_unique() {
        for m in ALL {
            assert_eq!(Method::from_name(m.name()), Some(m));
        }
        let mut names: Vec<_> = ALL.iter().map(|m| m.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ALL.len());
    }
}
