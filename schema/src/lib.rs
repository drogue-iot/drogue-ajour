use chrono::{DateTime, Utc};
use drogue_client::{core::v1::Conditions, dialect, Section};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum ImagePullPolicy {
    Always,
    IfNotPresent,
}

impl Default for ImagePullPolicy {
    fn default() -> Self {
        Self::IfNotPresent
    }
}

dialect!(FirmwareSpec [Section::Spec => "firmware"]);

#[derive(Serialize, Deserialize, Debug)]
pub enum FirmwareSpec {
    #[serde(rename = "oci")]
    OCI {
        image: String,
        #[serde(rename = "imagePullPolicy", default = "Default::default")]
        image_pull_policy: ImagePullPolicy,
        #[serde(skip_serializing_if = "Option::is_none")]
        build: Option<FirmwareBuildSpec>,
    },
    #[serde(rename = "hawkbit")]
    HAWKBIT { controller: String },
    #[serde(rename = "file")]
    FILE { name: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FirmwareBuildSpec {
    /// Build source
    pub source: FirmwareBuildSource,
    /// Builder image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Pipeline environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<FirmwareBuildEnv>>,
    /// Build command line arguments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    /// Build timeout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FirmwareBuildEnv {
    pub name: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum FirmwareBuildSource {
    #[serde(rename = "git")]
    GIT {
        uri: String,
        project: String,
        rev: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FirmwareBuildArtifact {
    pub path: String,
}

dialect!(FirmwareStatus [Section::Status => "firmware"]);

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct FirmwareStatus {
    pub conditions: Conditions,
    pub current: String,
    pub target: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BuildInfo {
    pub app: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
