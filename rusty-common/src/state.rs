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

