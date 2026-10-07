use std::fmt;

/// Every failure the simulator can report. No code path panics: errors bubble up to
/// `main`, which prints them and exits with a non-zero status.
#[derive(Debug)]
pub enum SimError {
    /// The scenario file could not be read from disk.
    ScenarioRead {
        path: String,
        source: std::io::Error,
    },
    /// The scenario file is not valid scenario YAML.
    ScenarioParse(serde_yaml::Error),
    /// A sensor event could not be COBS/postcard encoded.
    Encode(postcard::Error),
    /// The HIL serial port could not be opened.
    SerialOpen(serialport::Error),
    /// The MIDI backend could not be initialised.
    MidiInit(midir::InitError),
    /// The virtual MIDI port could not be created.
    MidiConnect(String),
    /// The Slint UI failed to start or run.
    Ui(slint::PlatformError),
}

impl fmt::Display for SimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimError::ScenarioRead { path, source } => {
                write!(f, "failed to read scenario file '{path}': {source}")
            }
            SimError::ScenarioParse(e) => write!(f, "failed to parse scenario YAML: {e}"),
            SimError::Encode(e) => write!(f, "failed to encode sensor event: {e}"),
            SimError::SerialOpen(e) => write!(f, "failed to open serial port: {e}"),
            SimError::MidiInit(e) => write!(f, "failed to create MIDI output: {e}"),
            SimError::MidiConnect(e) => write!(f, "failed to create virtual MIDI port: {e}"),
            SimError::Ui(e) => write!(f, "UI error: {e}"),
        }
    }
}

impl std::error::Error for SimError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SimError::ScenarioRead { source, .. } => Some(source),
            SimError::ScenarioParse(e) => Some(e),
            SimError::Encode(e) => Some(e),
            SimError::SerialOpen(e) => Some(e),
            SimError::MidiInit(e) => Some(e),
            SimError::Ui(e) => Some(e),
            SimError::MidiConnect(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    fn yaml_error() -> serde_yaml::Error {
        serde_yaml::from_str::<u8>("not a number").unwrap_err()
    }

    #[test]
    fn scenario_read_mentions_path_and_has_source() {
        let err = SimError::ScenarioRead {
            path: "missing.yml".into(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "gone"),
        };
        let msg = err.to_string();
        assert!(msg.contains("missing.yml"), "{msg}");
        assert!(msg.contains("gone"), "{msg}");
        assert!(err.source().is_some());
    }

    #[test]
    fn wrapped_errors_expose_their_source() {
        let errors = [
            SimError::ScenarioParse(yaml_error()),
            SimError::Encode(postcard::Error::SerializeBufferFull),
            SimError::SerialOpen(serialport::Error::new(
                serialport::ErrorKind::NoDevice,
                "no device",
            )),
            SimError::MidiInit(midir::InitError),
            SimError::Ui(slint::PlatformError::Other("boom".into())),
        ];
        for err in errors {
            assert!(err.source().is_some(), "{err:?} should have a source");
            assert!(!err.to_string().is_empty());
        }
    }

    #[test]
    fn leaf_errors_have_no_source() {
        let connect = SimError::MidiConnect("port busy".into());
        assert!(connect.source().is_none());
        assert!(connect.to_string().contains("port busy"));
    }
}
