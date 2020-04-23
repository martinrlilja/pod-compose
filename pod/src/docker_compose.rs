use serde::{Deserialize, Serialize};
use std::collections::BTreeMap as Map;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DockerComposeFile {
    pub version: String,
    pub services: Map<String, Service>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Service {
    pub replicas: Option<u64>,

    pub image: Option<String>,

    pub build: Option<Build>,

    #[serde(default)]
    pub ports: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Build {
    Short(String),
    Extended {
        context: String,

        dockerfile: Option<String>,

        #[serde(default)]
        args: MapList,

        #[serde(default)]
        cache_from: Vec<String>,

        #[serde(default)]
        labels: MapList,

        shm_size: Option<String>,

        target: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MapList {
    Map(Map<String, String>),
    List(Vec<String>),
}

impl Default for MapList {
    fn default() -> Self {
        MapList::List(Vec::new())
    }
}

impl MapList {
    pub fn to_map(self) -> Map<String, String> {
        match self {
            MapList::Map(map) => map,
            MapList::List(list) => list.into_iter().map(MapList::split_value).collect(),
        }
    }

    fn split_value(value: String) -> (String, String) {
        let split_index = value.find("=");
        match split_index {
            Some(split_index) => {
                let (key, value) = value.split_at(split_index);
                (key.into(), value.into())
            }
            None => (value.into(), "".into()),
        }
    }
}
