use anyhow::{anyhow, Result};
use std::{
    env,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

use composer_frontend::{ComposerFrontend, DockerComposeFrontend};
use container_backend::{ContainerBackend, PodmanBackend};

mod composer_frontend;
mod container_backend;
mod docker_compose;
mod hasher;
mod models;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "pod",
    about = "A docker-compose compatible tool for running containers with podman."
)]
enum Opt {
    Build {
        #[structopt(short, long)]
        pull: bool,
    },
    Down {
        #[structopt(short, long)]
        /// Also remove containers.
        volumes: bool,
    },
    /// Finds a docker-compose.yaml file and starts the containers defined in it.
    Up {
        #[structopt(short, long)]
        /// Start containers in the background.
        detach: bool,

        #[structopt(long)]
        /// Build images before starting the containers.
        build: bool,
    },
}

fn find_compose_file<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
    for path in path.as_ref().ancestors() {
        let docker_file_path = path.join("docker-compose.yml");
        if docker_file_path.exists() {
            return Some(docker_file_path);
        }

        let docker_file_path = path.join("docker-compose.yaml");
        if docker_file_path.exists() {
            return Some(docker_file_path);
        }
    }

    None
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    let current_dir = env::current_dir()?;
    let compose_file_path = find_compose_file(current_dir);

    let compose_file_path = compose_file_path
        .ok_or_else(|| anyhow!("Couldn't find a docker-compose.yml file in the current working directory or any of its parents."))?;

    let work_directory = compose_file_path
        .parent()
        .ok_or_else(|| anyhow!("Docker compose file has no parent."))?;

    env::set_current_dir(work_directory)?;

    let project_name = work_directory
        .file_name()
        .and_then(|path| path.to_str())
        .ok_or_else(|| anyhow!("Couldn't determine the project name."))?;

    let mut frontend = DockerComposeFrontend::new();
    let composition = frontend.composition(project_name, &compose_file_path)?;

    let mut backend = PodmanBackend::connect()?;

    match opt {
        Opt::Build { pull: _ } => (),
        Opt::Down { volumes: _ } => (),
        Opt::Up { detach: _, build } => {
            for image_spec in composition.images {
                if !build && backend.image_exists(&image_spec.image_name)? {
                    continue;
                }

                println!("Building image {}", image_spec.image_name);
                backend.build_image(image_spec)?;
            }

            for container_spec in composition.containers {
                let container_name = container_spec.container_name.clone();
                let container_exists = backend.container_exists(&container_spec.container_name)?;

                if !container_exists {
                    println!("Creating container {}", container_name);
                    backend.create_container(container_spec)?;
                }

                println!("Starting container {}", container_name);
                backend.start_container(&container_name)?;
            }
        }
    }

    Ok(())
}
