/// Scenario used when no `.yml` file is given on the command line.
pub const DEFAULT_SCENARIO: &str = "simulator/scenario.yml";

/// Parsed command-line arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    /// Path of the YAML scenario to replay.
    pub scenario_path: String,
    /// Serial device (e.g. `/dev/ttyACM0`) enabling Hardware-In-the-Loop mode.
    pub serial_port: Option<String>,
}

impl CliArgs {
    /// Parses `std::env::args()`-like input. The first item (program name) is skipped.
    pub fn parse<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let args: Vec<String> = args.into_iter().skip(1).map(Into::into).collect();
        let scenario_path = args
            .iter()
            .find(|a| a.ends_with(".yml"))
            .cloned()
            .unwrap_or_else(|| DEFAULT_SCENARIO.to_string());
        let serial_port = args.iter().find(|a| a.starts_with("/dev/")).cloned();
        Self {
            scenario_path,
            serial_port,
        }
    }

    /// The theremin instrument (and its UI) is selected by the scenario file name.
    pub fn is_theremin(&self) -> bool {
        self.scenario_path.contains("theremin")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_without_arguments() {
        let args = CliArgs::parse(["simulator"]);
        assert_eq!(args.scenario_path, DEFAULT_SCENARIO);
        assert_eq!(args.serial_port, None);
        assert!(!args.is_theremin());
    }

    #[test]
    fn empty_iterator_is_handled() {
        let args = CliArgs::parse(Vec::<String>::new());
        assert_eq!(args.scenario_path, DEFAULT_SCENARIO);
        assert_eq!(args.serial_port, None);
    }

    #[test]
    fn picks_scenario_and_serial_port_in_any_order() {
        let args = CliArgs::parse(["sim", "/dev/ttyACM0", "simulator/theremin_scenario.yml"]);
        assert_eq!(args.scenario_path, "simulator/theremin_scenario.yml");
        assert_eq!(args.serial_port.as_deref(), Some("/dev/ttyACM0"));
        assert!(args.is_theremin());
    }

    #[test]
    fn program_name_is_never_taken_as_scenario() {
        let args = CliArgs::parse(["weird.yml"]);
        assert_eq!(args.scenario_path, DEFAULT_SCENARIO);
    }

    #[test]
    fn first_matching_argument_wins() {
        let args = CliArgs::parse(["sim", "a.yml", "b.yml", "/dev/one", "/dev/two"]);
        assert_eq!(args.scenario_path, "a.yml");
        assert_eq!(args.serial_port.as_deref(), Some("/dev/one"));
    }
}
