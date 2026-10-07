use std::path::Path;

use expressive_core::SensorEvent;
use serde::{Deserialize, Serialize};

use crate::error::SimError;

/// One step of a scenario: an optional sensor event, then an optional pause.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulatorAction {
    pub event: Option<SensorEvent>,
    pub wait_ms: Option<u64>,
}

/// A replayable sequence of sensor events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scenario {
    pub actions: Vec<SimulatorAction>,
}

impl Scenario {
    /// Parses a scenario from its YAML representation.
    pub fn from_yaml(yaml: &str) -> Result<Self, SimError> {
        serde_yaml::from_str(yaml).map_err(SimError::ScenarioParse)
    }

    /// Reads and parses a scenario file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SimError> {
        let path = path.as_ref();
        let yaml = std::fs::read_to_string(path).map_err(|source| SimError::ScenarioRead {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_yaml(&yaml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

    #[test]
    fn parses_events_and_waits() {
        let yaml =
            "actions:\n  - event: !Position3D { id: 0, x: 1, y: 2, z: 300 }\n  - wait_ms: 50\n";
        let scenario = Scenario::from_yaml(yaml).unwrap();
        assert_eq!(
            scenario.actions,
            vec![
                SimulatorAction {
                    event: Some(SensorEvent::Position3D {
                        id: 0,
                        x: 1,
                        y: 2,
                        z: 300
                    }),
                    wait_ms: None,
                },
                SimulatorAction {
                    event: None,
                    wait_ms: Some(50),
                },
            ]
        );
    }

    #[test]
    fn rejects_invalid_yaml() {
        let err = Scenario::from_yaml("actions: 42").unwrap_err();
        assert!(matches!(err, SimError::ScenarioParse(_)), "{err:?}");

        let err = Scenario::from_yaml("actions:\n  - event: !Teleport { id: 0 }\n").unwrap_err();
        assert!(matches!(err, SimError::ScenarioParse(_)), "{err:?}");
    }

    #[test]
    fn missing_file_reports_its_path() {
        let err = Scenario::load("does/not/exist.yml").unwrap_err();
        match err {
            SimError::ScenarioRead { path, .. } => assert_eq!(path, "does/not/exist.yml"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn shipped_mpe_scenario_is_valid() {
        let scenario = Scenario::load(format!("{MANIFEST_DIR}/scenario.yml")).unwrap();
        assert!(scenario.actions.iter().any(|a| a.event.is_some()));
    }

    #[test]
    fn shipped_theremin_scenario_is_valid() {
        let scenario = Scenario::load(format!("{MANIFEST_DIR}/theremin_scenario.yml")).unwrap();
        assert!(scenario
            .actions
            .iter()
            .any(|a| matches!(a.event, Some(SensorEvent::Position3D { .. }))));
    }

    #[test]
    fn yaml_round_trip() {
        let scenario = Scenario {
            actions: vec![SimulatorAction {
                event: Some(SensorEvent::Released { id: 3 }),
                wait_ms: Some(10),
            }],
        };
        let yaml = serde_yaml::to_string(&scenario).unwrap();
        assert_eq!(Scenario::from_yaml(&yaml).unwrap(), scenario);
    }
}
