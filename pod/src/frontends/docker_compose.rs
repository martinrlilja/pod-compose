use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::{
    collections::BTreeMap as Map,
    fs::File,
    path::{Path, PathBuf},
};

use crate::{
    models::{Composition, ContainerName, ContainerSpec, ImageSpec},
    services::ComposerFrontend,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DockerComposeFile {
    pub version: String,
    pub services: Map<String, Service>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Service {
    pub replicas: Option<u64>,

    pub image: Option<String>,

    pub build: Option<Build>,

    #[serde(default)]
    pub ports: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Build {
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
enum MapList {
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

pub struct DockerComposeFrontend;

impl DockerComposeFrontend {
    pub fn new() -> DockerComposeFrontend {
        DockerComposeFrontend
    }
}

impl ComposerFrontend for DockerComposeFrontend {
    fn composition<P: AsRef<Path>>(
        &mut self,
        project_name: &str,
        compose_file_path: P,
    ) -> Result<Composition> {
        let compose_file_path = compose_file_path.as_ref();
        let compose_file = File::open(&compose_file_path)?;

        let file: DockerComposeFile = serde_yaml::from_reader(compose_file)?;
        let mut composition: Composition = Default::default();

        for (service_name, service) in file.services {
            let image_name = match service.image {
                Some(image_name) => image_name,
                None => format!("{}_{}", project_name, service_name),
            };

            match service.build {
                Some(Build::Short(context)) => {
                    let image_spec = ImageSpec {
                        context: PathBuf::from(context),
                        dockerfile: PathBuf::from("Dockerfile"),
                        target: None,
                        build_args: Default::default(),
                        image_name: image_name.clone(),
                    };
                    composition.images.push(image_spec);
                }
                Some(Build::Extended {
                    context,
                    dockerfile,
                    args,
                    target,
                    ..
                }) => {
                    let image_spec = ImageSpec {
                        context: PathBuf::from(context),
                        dockerfile: PathBuf::from(
                            dockerfile.unwrap_or_else(|| "Dockerfile".into()),
                        ),
                        target: target,
                        build_args: args.to_map(),
                        image_name: image_name.clone(),
                    };
                    composition.images.push(image_spec);
                }
                None => (),
            }

            for index in 0..service.replicas.unwrap_or(1) {
                let container = ContainerSpec {
                    service_name: service_name.clone(),
                    image_name: image_name.clone(),
                    container_name: ContainerName(format!(
                        "{}_{}_{}",
                        project_name, service_name, index
                    )),
                    labels: Default::default(),
                };
                composition.containers.push(container);
            }
        }

        Ok(composition)
    }
}
