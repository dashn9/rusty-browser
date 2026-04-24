use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserState {
    Idle,
    Reserved,
    PartialReserved,
}

impl BrowserState {
    pub fn as_str(&self) -> &'static str {
        match self {
            BrowserState::Idle => "idle",
            BrowserState::Reserved => "reserved",
            BrowserState::PartialReserved => "partial_reserved",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "reserved" => BrowserState::Reserved,
            "partial_reserved" => BrowserState::PartialReserved,
            _ => BrowserState::Idle,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserInfo {
    pub browser_id: String,
    pub execution_id: String,
    pub public_ip: String,
    pub private_ip: String,
    pub grpc_port: u16,
    pub state: BrowserState,
    pub contexts: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_state_as_str_idle() {
        assert_eq!(BrowserState::Idle.as_str(), "idle");
    }

    #[test]
    fn browser_state_as_str_reserved() {
        assert_eq!(BrowserState::Reserved.as_str(), "reserved");
    }

    #[test]
    fn browser_state_as_str_partial_reserved() {
        assert_eq!(BrowserState::PartialReserved.as_str(), "partial_reserved");
    }

    #[test]
    fn browser_state_from_str_reserved() {
        assert_eq!(BrowserState::from_str("reserved"), BrowserState::Reserved);
    }

    #[test]
    fn browser_state_from_str_partial_reserved() {
        assert_eq!(BrowserState::from_str("partial_reserved"), BrowserState::PartialReserved);
    }

    #[test]
    fn browser_state_from_str_idle_explicit() {
        assert_eq!(BrowserState::from_str("idle"), BrowserState::Idle);
    }

    #[test]
    fn browser_state_from_str_unknown_falls_back_to_idle() {
        assert_eq!(BrowserState::from_str("garbage"), BrowserState::Idle);
        assert_eq!(BrowserState::from_str(""), BrowserState::Idle);
        assert_eq!(BrowserState::from_str("RESERVED"), BrowserState::Idle);
    }

    #[test]
    fn browser_state_round_trip_via_str() {
        for state in [BrowserState::Idle, BrowserState::Reserved, BrowserState::PartialReserved] {
            assert_eq!(BrowserState::from_str(state.as_str()), state);
        }
    }

    #[test]
    fn browser_state_serde_round_trip() {
        let cases = [
            (BrowserState::Idle, "\"idle\""),
            (BrowserState::Reserved, "\"reserved\""),
            (BrowserState::PartialReserved, "\"partial_reserved\""),
        ];
        for (state, expected_json) in cases {
            let serialized = serde_json::to_string(&state).unwrap();
            assert_eq!(serialized, expected_json);
            let deserialized: BrowserState = serde_json::from_str(&serialized).unwrap();
            assert_eq!(deserialized, state);
        }
    }

    #[test]
    fn browser_info_fields_are_public() {
        let info = BrowserInfo {
            browser_id: "b1".to_string(),
            execution_id: "e1".to_string(),
            public_ip: "1.2.3.4".to_string(),
            private_ip: "10.0.0.1".to_string(),
            grpc_port: 9090,
            state: BrowserState::Idle,
            contexts: vec!["ctx1".to_string()],
        };
        assert_eq!(info.browser_id, "b1");
        assert_eq!(info.execution_id, "e1");
        assert_eq!(info.public_ip, "1.2.3.4");
        assert_eq!(info.private_ip, "10.0.0.1");
        assert_eq!(info.grpc_port, 9090);
        assert_eq!(info.state, BrowserState::Idle);
        assert_eq!(info.contexts, vec!["ctx1"]);
    }

    #[test]
    fn browser_info_serde_round_trip() {
        let info = BrowserInfo {
            browser_id: "b-abc".to_string(),
            execution_id: "exec-123".to_string(),
            public_ip: "5.6.7.8".to_string(),
            private_ip: "192.168.1.1".to_string(),
            grpc_port: 50051,
            state: BrowserState::Reserved,
            contexts: vec!["ctx-a".to_string(), "ctx-b".to_string()],
        };
        let json = serde_json::to_string(&info).unwrap();
        let restored: BrowserInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.browser_id, info.browser_id);
        assert_eq!(restored.execution_id, info.execution_id);
        assert_eq!(restored.grpc_port, info.grpc_port);
        assert_eq!(restored.state, BrowserState::Reserved);
        assert_eq!(restored.contexts.len(), 2);
    }

    #[test]
    fn browser_info_empty_contexts() {
        let info = BrowserInfo {
            browser_id: "b1".to_string(),
            execution_id: "e1".to_string(),
            public_ip: "0.0.0.0".to_string(),
            private_ip: "0.0.0.0".to_string(),
            grpc_port: 1234,
            state: BrowserState::Idle,
            contexts: vec![],
        };
        assert!(info.contexts.is_empty());
    }
}
