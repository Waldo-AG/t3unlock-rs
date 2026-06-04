//! Supported SSD model variants.

use serde::Serialize;

/// Supported SSD models
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Model {
    T1,
    T3,
    T5,
}

impl Model {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "t1" => Some(Model::T1),
            "t3" => Some(Model::T3),
            "t5" => Some(Model::T5),
            _ => None,
        }
    }

    pub fn locked_pid(self) -> u16 {
        match self {
            Model::T1 => 0x61f2,
            Model::T3 => 0x61f4,
            Model::T5 => 0x61f6,
        }
    }

    pub fn normal_pid(self) -> u16 {
        match self {
            Model::T1 => 0x61f1,
            Model::T3 => 0x61f3,
            Model::T5 => 0x61f5,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Model::T1 => "T1",
            Model::T3 => "T3",
            Model::T5 => "T5",
        }
    }
}
