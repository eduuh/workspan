//! `workspan agent pause` / `unpause`: the switch behind [`bn_core::agent_pause`].

/// Pause agent launches with `reason`, or unpause them when `reason` is `None`.
pub(crate) fn emit_pause(reason: Option<&str>, json: bool) {
    let result = match reason {
        Some(reason) => bn_core::agent_pause::pause(reason).map(|()| true),
        None => bn_core::agent_pause::unpause(),
    };
    let changed = result.unwrap_or_else(|error| {
        crate::fail_json(json, "agent_pause", "io", &format!("workspan agent pause: {error}"))
    });
    let paused = reason.is_some();
    if json {
        let data = serde_json::json!({ "paused": paused, "changed": changed });
        println!("{}", crate::Envelope::success("agent_pause", data).to_json());
    } else if paused {
        println!("agents paused: workspan will not start agent sessions until `workspan agent unpause`");
    } else if changed {
        println!("agents unpaused");
    } else {
        println!("agents were not paused");
    }
}
