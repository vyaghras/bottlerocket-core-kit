/*!
# API models

Bottlerocket has different variants supporting different features and use cases.
Each variant has its own set of software, and therefore needs its own configuration.
We support having an API model for each variant to support these different configurations.

The model here defines a top-level `Settings` structure, and delegates the actual implementation to a ["settings plugin"](https://github.com/bottlerocket/bottlerocket-settings-sdk/tree/settings-plugins).
Settings plugin are written in Rust as a "cdylib" crate, and loaded at runtime.

Each settings plugin must define its own private `Settings` structure.
It can use pre-defined structures inside, or custom ones as needed.

`apiserver::datastore` offers serialization and deserialization modules that make it easy to map between Rust types and the data store, and thus, all inputs and outputs are type-checked.

At the field level, standard Rust types can be used, or ["modeled types"](src/modeled_types) that add input validation.

The `#[model]` attribute on Settings and its sub-structs reduces duplication and adds some required metadata; see [its docs](model-derive/) for details.
*/

// Types used to communicate between client and server for 'apiclient exec'.
pub mod exec;

// Types used to communicate between client and server for 'apiclient ephemeral-storage'.
pub mod ephemeral_storage;

use bottlerocket_release::BottlerocketRelease;
use bottlerocket_settings_models::model_derive::model;
use bottlerocket_settings_plugin::BottlerocketSettings;
use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use serde_plain::derive_fromstr_from_deserialize;
use std::{collections::HashMap, fmt};

use bottlerocket_settings_models::modeled_types::SingleLineString;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Settings {
    inner: BottlerocketSettings,
}

// This is the top-level model exposed by the API system. It contains the common sections for all
// variants.  This allows a single API call to retrieve everything the API system knows, which is
// useful as a check and also, for example, as a data source for templated configuration files.
#[model]
pub struct Model {
    settings: Settings,
    services: Services,
    configuration_files: ConfigurationFiles,
    os: BottlerocketRelease,
}

///// Internal services

// Note: Top-level objects that get returned from the API should have a "rename" attribute
// matching the struct name, but in kebab-case, e.g. ConfigurationFiles -> "configuration-files".
// This lets it match the datastore name.
// Objects that live inside those top-level objects, e.g. Service lives in Services, should have
// rename="" so they don't add an extra prefix to the datastore path that doesn't actually exist.
// This is important because we have APIs that can return those sub-structures directly.

pub type Services = HashMap<String, Service>;

#[model(add_option = false, rename = "")]
struct Service {
    configuration_files: Vec<SingleLineString>,
    restart_commands: Vec<String>,
}

pub type ConfigurationFiles = HashMap<String, ConfigurationFile>;

#[model(add_option = false, rename = "")]
struct ConfigurationFile {
    path: SingleLineString,
    template_path: SingleLineString,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
}

///// Metadata

#[model(add_option = false, rename = "metadata")]
struct Metadata {
    key: SingleLineString,
    md: SingleLineString,
    val: toml::Value,
}

#[model(add_option = false)]
struct Report {
    name: String,
    description: String,
}

/// Weak settings are ephemeral and deleted on reboot, regardless of whether or not it
/// is written by a setting generator.
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum Strength {
    #[default]
    Strong,
    Weak,
}

impl std::fmt::Display for Strength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Strength::Strong => write!(f, "strong"),
            Strength::Weak => write!(f, "weak"),
        }
    }
}

derive_fromstr_from_deserialize!(Strength);

/// Struct to hold the setting generator definition containing
/// command, strength, skip-if-populated
#[derive(Default, Serialize, std::fmt::Debug, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SettingsGenerator {
    pub command: String,
    #[serde(default = "Strength::default")]
    pub strength: Strength,
    pub skip_if_populated: bool,
}

impl SettingsGenerator {
    pub fn is_weak(&self) -> bool {
        self.strength == Strength::Weak
    }
}

impl<'de> Deserialize<'de> for SettingsGenerator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SettingsGeneratorVisitor;
        impl<'de> Visitor<'de> for SettingsGeneratorVisitor {
            type Value = SettingsGenerator;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or a map")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                // If the value is a string, use it as the `command` with defaults for other fields.
                Ok(SettingsGenerator {
                    command: value.to_string(),
                    strength: Strength::default(),
                    skip_if_populated: true,
                })
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                // Extract values from the map
                let mut command = None;
                let mut strength = None;
                let mut skip_if_populated = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "command" => command = Some(map.next_value()?),
                        "strength" => strength = Some(map.next_value()?),
                        "skip-if-populated" => skip_if_populated = Some(map.next_value()?),
                        _ => {
                            return Err(de::Error::unknown_field(
                                &key,
                                &["command", "strength", "skip-if-populated"],
                            ))
                        }
                    }
                }
                Ok(SettingsGenerator {
                    command: command.ok_or_else(|| de::Error::missing_field("command"))?,
                    strength: strength.unwrap_or_default(),
                    skip_if_populated: skip_if_populated.unwrap_or(true),
                })
            }
        }
        deserializer.deserialize_any(SettingsGeneratorVisitor)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_setting_generator_deserialization() {
        let api_response = r#"
            {
                "host-containers.admin.source": "generator1",
                "host-containers.control.source": {
                    "command": "generator2",
                    "strength": "weak",
                    "skip-if-populated": true
                },
                "host-containers.no_skip_if_populated.source": {
                    "command": "generator3",
                    "strength": "weak"
                },
                "host-containers.skip_if_populated_given.source": {
                    "command": "generator3",
                    "strength": "weak",
                    "skip-if-populated": false
                }
            }"#;

        let expected_admin = SettingsGenerator {
            command: "generator1".to_string(),
            strength: Strength::Strong,
            skip_if_populated: true,
        };

        let expected_control = SettingsGenerator {
            command: "generator2".to_string(),
            strength: Strength::Weak,
            skip_if_populated: true,
        };

        let expected_no_skip_if_populated = SettingsGenerator {
            command: "generator3".to_string(),
            strength: Strength::Weak,
            skip_if_populated: true,
        };

        let expected_skip_if_populated_given = SettingsGenerator {
            command: "generator3".to_string(),
            strength: Strength::Weak,
            skip_if_populated: false,
        };

        let result: HashMap<String, SettingsGenerator> =
            serde_json::from_str(api_response).unwrap();

        assert_eq!(
            result.get("host-containers.admin.source").unwrap(),
            &expected_admin
        );
        assert_eq!(
            result.get("host-containers.control.source").unwrap(),
            &expected_control
        );
        assert_eq!(
            result
                .get("host-containers.no_skip_if_populated.source")
                .unwrap(),
            &expected_no_skip_if_populated
        );
        assert_eq!(
            result
                .get("host-containers.skip_if_populated_given.source")
                .unwrap(),
            &expected_skip_if_populated_given
        );
    }
}
