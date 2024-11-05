//! The 'generator' module holds types that handles the settings generator metadata
//! definition (containing command, strength and depth) among various systems
//! like apiserver, sundog, datastore.
//!
//! The command field defines the command that needs to be executed to populate the
//! setting.
//! The strength field defines whether a setting needs to be deleted on reboot.
//! The depth field defines how metadata is inherited across hierarchical levels,
//! allowing a parent to provide metadata that can be applied at children at a given depth.
//!
//! SettingsGenerator type is used to hold generator that is applied strictly
//! to the given setting and have depth 0.
//! The RawSettingsGenerator holds the generators that can be dynamically applied
//! to the successors at the given depth where a depth '0' means the generator
//! should be applied on the given key.
//!
//! We use a custom deserializer because the metadata may not always be
//! structured as an object; it can also appear as a string. This deserializer
//! handles both formats, keeping the deserialization logic close to the struct
//! for maintainability and clarity.

use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use serde_plain::derive_fromstr_from_deserialize;
use std::fmt::{self, Display};

/// Weak settings are ephemeral and deleted on reboot, regardless of whether or not it
/// is written by a setting generator.
#[derive(Default, Deserialize, Serialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Strength {
    #[default]
    Strong,
    Weak,
}

impl Display for Strength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Strength::Strong => write!(f, "strong"),
            Strength::Weak => write!(f, "weak"),
        }
    }
}

derive_fromstr_from_deserialize!(Strength);

/// Struct to hold the setting generator definition containing
/// command, strength, depth
#[derive(Clone, Default, Serialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RawSettingsGenerator {
    pub command: String,
    pub strength: Strength,
    pub depth: u32,
}

impl RawSettingsGenerator {
    pub fn is_weak(&self) -> bool {
        self.strength == Strength::Weak
    }
}

impl<'de> Deserialize<'de> for RawSettingsGenerator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SettingsGeneratorVisitor;
        impl<'de> Visitor<'de> for SettingsGeneratorVisitor {
            type Value = RawSettingsGenerator;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or a map")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                // If the value is a string, use it as the `command` with defaults for other fields.
                Ok(RawSettingsGenerator {
                    command: value.to_string(),
                    ..RawSettingsGenerator::default()
                })
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                // Extract values from the map
                let mut command = None;
                let mut strength = None;
                let mut depth = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "command" => command = Some(map.next_value()?),
                        "strength" => strength = Some(map.next_value()?),
                        "depth" => depth = Some(map.next_value()?),
                        _ => {
                            return Err(de::Error::unknown_field(
                                &key,
                                &["command", "strength", "depth"],
                            ))
                        }
                    }
                }
                Ok(RawSettingsGenerator {
                    command: command.ok_or_else(|| de::Error::missing_field("command"))?,
                    strength: strength.unwrap_or_default(),
                    depth: depth.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_any(SettingsGeneratorVisitor)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_setting_generator_deserialization() {
        let api_response = r#"
            {
                "host-containers.admin.source": "generator1",
                "host-containers.control.source": {
                    "command": "generator2",
                    "strength": "weak",
                    "depth": 0
                },
                "host-containers.no_depth.source": {
                    "command": "generator3",
                    "strength": "weak"
                },
                "host-containers.depth_given.source": {
                    "command": "generator4",
                    "strength": "weak",
                    "depth": 1
                }
            }"#;

        let expected_admin = RawSettingsGenerator {
            command: "generator1".to_string(),
            strength: Strength::Strong,
            depth: 0,
        };

        let expected_control = RawSettingsGenerator {
            command: "generator2".to_string(),
            strength: Strength::Weak,
            depth: 0,
        };

        let expected_no_depth = RawSettingsGenerator {
            command: "generator3".to_string(),
            strength: Strength::Weak,
            depth: 0,
        };

        let expected_depth_given = RawSettingsGenerator {
            command: "generator4".to_string(),
            strength: Strength::Weak,
            depth: 1,
        };

        let result: HashMap<String, RawSettingsGenerator> =
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
            result.get("host-containers.no_depth.source").unwrap(),
            &expected_no_depth
        );
        assert_eq!(
            result.get("host-containers.depth_given.source").unwrap(),
            &expected_depth_given
        );
    }
}

/// Struct to hold the setting generator definition containing
/// command, strength
#[derive(Default, Serialize, std::fmt::Debug, PartialEq)]
pub struct SettingsGenerator {
    pub command: String,
    pub strength: Strength,
}

impl From<RawSettingsGenerator> for SettingsGenerator {
    fn from(value: RawSettingsGenerator) -> Self {
        SettingsGenerator {
            command: value.command,
            strength: value.strength,
        }
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
                    ..SettingsGenerator::default()
                })
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                // Extract values from the map
                let mut command = None;
                let mut strength = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "command" => command = Some(map.next_value()?),
                        "strength" => strength = Some(map.next_value()?),
                        _ => return Err(de::Error::unknown_field(&key, &["command", "strength"])),
                    }
                }
                Ok(SettingsGenerator {
                    command: command.ok_or_else(|| de::Error::missing_field("command"))?,
                    strength: strength.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_any(SettingsGeneratorVisitor)
    }
}
