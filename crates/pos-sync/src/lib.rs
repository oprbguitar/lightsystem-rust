use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum SyncState {
    #[default]
    Standalone,
    LocalServiceConfigured {
        address: String,
    },
}

impl SyncState {
    pub fn from_optional_address(address: Option<String>) -> Self {
        address
            .filter(|value| !value.trim().is_empty())
            .map_or(Self::Standalone, |address| Self::LocalServiceConfigured {
                address,
            })
    }
    pub fn is_implemented_for_first_release(&self) -> bool {
        matches!(self, Self::Standalone)
    }
}

#[cfg(test)]
mod tests {
    use super::SyncState;
    #[test]
    fn blank_address_stays_standalone_and_remote_is_marked_configured_not_ready() {
        assert!(
            SyncState::from_optional_address(Some("  ".into())).is_implemented_for_first_release()
        );
        assert!(
            !SyncState::from_optional_address(Some("127.0.0.1:8080".into()))
                .is_implemented_for_first_release()
        );
    }
}
