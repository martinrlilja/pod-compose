use anyhow::{anyhow, Result};
use ignore::WalkBuilder;
use number_prefix::NumberPrefix;
use std::{collections::BTreeMap as Map, fs::OpenOptions};
use tar::Builder as TarBuilder;
use tempfile::TempDir;
use varlink::Connection;

use podman_varlink::{
    AuthConfig, BuildInfo, Create as CreateContainer, VarlinkClient, VarlinkClientInterface,
    ErrorKind, Error
};

use crate::{
    models::{
        Container, ContainerId, ContainerName, ContainerSpec, ContainerStatus, Image,
        ImageBuildSpec, ImageId, ImageName,
    },
    services::ContainerBackend,
};

pub struct PodmanBackend {
    client: VarlinkClient,
}

impl PodmanBackend {
    pub fn connect() -> Result<PodmanBackend> {
        let connection = Connection::with_activate(r#"podman varlink "$VARLINK_ADDRESS""#)?;
        let client = VarlinkClient::new(connection.clone());

        Ok(PodmanBackend { client })
    }
}

impl ContainerBackend for PodmanBackend {
    fn get_image(&mut self, name: &ImageName) -> Result<Option<Image>> {
        let reply = self.client.get_image(name.0.clone()).call();

        let reply = match reply {
            Ok(reply) => reply,
            Err(Error(ErrorKind::ImageNotFound(_), _, _)) => return Ok(None),
            Err(err) => Err(err)?,
        };

        let labels = reply.image
            .labels
            .map(|labels| labels.into_iter().collect())
            .unwrap_or_else(Default::default);

        Ok(Some(Image {
            id: ImageId(reply.image.id),
            labels,
        }))
    }

    fn pull_image(&mut self, name: &ImageName) -> Result<ImageId> {
        let auth_config = AuthConfig {
            username: None,
            password: None,
        };

        let mut image_id = None;

        for reply in self.client.pull_image(name.0.clone(), auth_config).more()? {
            let reply = reply?.reply;

            if let Some(logs) = reply.logs {
                if !logs.is_empty() {
                    for line in logs {
                        print!("{}", line);
                    }
                }
            }

            if !reply.id.is_empty() {
                image_id = Some(reply.id);
            }
        }

        let image_id = ImageId(image_id.unwrap());
        Ok(image_id)
    }

    fn build_image(&mut self, spec: ImageBuildSpec) -> Result<ImageId> {
        let temp_dir = TempDir::new()?;
        let temp_context_path = temp_dir.path().join("context.tar");
        let temp_context = {
            let mut options = OpenOptions::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options.write(true).create(true).open(&temp_context_path)?
        };

        let mut tar = TarBuilder::new(temp_context);
        let walk = WalkBuilder::new(spec.context)
            .add_custom_ignore_filename(".dockerignore")
            .ignore(false)
            .git_global(false)
            .git_ignore(false)
            .git_exclude(false)
            .hidden(false)
            .build();

        let mut context_size = 0;
        for result in walk {
            let result = result?;
            tar.append_path(result.path())?;
            context_size += result.metadata()?.len();
        }

        match NumberPrefix::binary(context_size as f32) {
            NumberPrefix::Standalone(bytes) => println!("Archived build context ({} bytes)", bytes),
            NumberPrefix::Prefixed(prefix, n) => {
                println!("Archived build context ({:.1} {}B)", n, prefix)
            }
        };

        tar.finish()?;

        let context_dir = temp_context_path
            .to_str()
            .ok_or_else(|| anyhow!("the canonical context path is not valid utf-8"))?;

        let dockerfile = spec.dockerfile.canonicalize()?;
        let dockerfile = dockerfile
            .to_str()
            .ok_or_else(|| anyhow!("the canonical dockerfile path is not valid utf-8"))?;

        let build_info = BuildInfo {
            architecture: None,
            addCapabilities: None,
            additionalTags: None,
            annotations: None,
            buildArgs: None,
            buildOptions: None,
            cniConfigDir: None,
            cniPluginDir: None,
            compression: None,
            contextDir: context_dir.into(),
            defaultsMountFilePath: None,
            devices: None,
            dockerfiles: vec![dockerfile.into()],
            dropCapabilities: None,
            err: None,
            forceRmIntermediateCtrs: None,
            iidfile: None,
            label: None,
            layers: None,
            nocache: Some(false),
            os: None,
            out: None,
            output: spec.name.0,
            outputFormat: None,
            pullPolicy: None,
            quiet: None,
            remoteIntermediateCtrs: None,
            reportWriter: None,
            runtimeArgs: None,
            signBy: None,
            squash: None,
            target: spec.target,
            transientMounts: None,
        };

        let mut image_id = None;

        for reply in self.client.build_image(build_info).more()? {
            let reply = reply?;

            if let Some(logs) = reply.image.logs {
                if !logs.is_empty() {
                    for line in logs {
                        print!("{}", line);
                    }
                }
            }

            if !reply.image.id.is_empty() {
                image_id = Some(reply.image.id);
            }
        }

        temp_dir.close()?;

        let image = ImageId(image_id.unwrap());
        Ok(image)
    }

    fn list_containers(
        &mut self,
        labels: Vec<(&str, &str)>,
    ) -> Result<Map<ContainerName, Container>> {
        let mut containers = Map::new();

        let reply = self.client.list_containers().call()?;

        let reply_containers = match reply.containers {
            Some(containers) => containers,
            None => return Ok(containers),
        };

        'container_loop: for container in reply_containers {
            let container_labels = container.labels.unwrap_or_else(Default::default);
            for (label, value) in labels.iter() {
                let container_label_value = container_labels.get(*label).map(|s| s.as_str());
                if container_label_value != Some(value) {
                    continue 'container_loop;
                }
            }

            let container_status = match container.status.as_str() {
                "configured" => ContainerStatus::Configured,
                "running" => ContainerStatus::Running,
                "exited" => ContainerStatus::Exited,
                status => {
                    eprintln!("Unknown container status: {:?}", status);
                    ContainerStatus::Unknown
                }
            };

            let container = Container {
                id: ContainerId(container.id),
                name: ContainerName(container.names),
                status: container_status,
                labels: container_labels.into_iter().collect(),
            };
            containers.insert(container.name.clone(), container);
        }

        Ok(containers)
    }

    fn create_container(&mut self, spec: ContainerSpec) -> Result<ContainerId> {
        let labels = spec
            .labels
            .into_iter()
            .map(|(key, value)| format!("{}={}", key, value))
            .collect();

        let create_container = CreateContainer {
            args: vec![spec.image_name.0],
            addHost: Default::default(),
            annotation: Default::default(),
            attach: Default::default(),
            blkioWeight: Default::default(),
            blkioWeightDevice: Default::default(),
            capAdd: Default::default(),
            capDrop: Default::default(),
            cgroupParent: Default::default(),
            cidFile: Default::default(),
            conmonPidfile: Default::default(),
            command: Default::default(),
            cpuPeriod: Default::default(),
            cpuQuota: Default::default(),
            cpuRtPeriod: Default::default(),
            cpuRtRuntime: Default::default(),
            cpuShares: Default::default(),
            cpus: Default::default(),
            cpuSetCpus: Default::default(),
            cpuSetMems: Default::default(),
            detach: Default::default(),
            detachKeys: Default::default(),
            device: Default::default(),
            deviceReadBps: Default::default(),
            deviceReadIops: Default::default(),
            deviceWriteBps: Default::default(),
            deviceWriteIops: Default::default(),
            dns: Default::default(),
            dnsOpt: Default::default(),
            dnsSearch: Default::default(),
            dnsServers: Default::default(),
            entrypoint: Default::default(),
            env: Default::default(),
            envFile: Default::default(),
            expose: Default::default(),
            gidmap: Default::default(),
            groupadd: Default::default(),
            healthcheckCommand: Default::default(),
            healthcheckInterval: Default::default(),
            healthcheckRetries: Default::default(),
            healthcheckStartPeriod: Default::default(),
            healthcheckTimeout: Default::default(),
            hostname: Default::default(),
            imageVolume: Default::default(),
            init: Default::default(),
            initPath: Default::default(),
            interactive: Default::default(),
            ip: Default::default(),
            ipc: Default::default(),
            kernelMemory: Default::default(),
            label: Some(labels),
            labelFile: Default::default(),
            logDriver: Default::default(),
            logOpt: Default::default(),
            macAddress: Default::default(),
            memory: Default::default(),
            memoryReservation: Default::default(),
            memorySwap: Default::default(),
            memorySwappiness: Default::default(),
            name: Some(spec.name.0),
            network: Default::default(),
            noHosts: Default::default(),
            oomKillDisable: Default::default(),
            oomScoreAdj: Default::default(),
            overrideArch: Default::default(),
            overrideOS: Default::default(),
            pid: Default::default(),
            pidsLimit: Default::default(),
            pod: Default::default(),
            privileged: Default::default(),
            publish: Default::default(),
            publishAll: Default::default(),
            pull: Default::default(),
            quiet: Default::default(),
            readonly: Default::default(),
            readonlytmpfs: Default::default(),
            restart: Default::default(),
            rm: Default::default(),
            rootfs: Default::default(),
            securityOpt: Default::default(),
            shmSize: Default::default(),
            stopSignal: Default::default(),
            stopTimeout: Default::default(),
            storageOpt: Default::default(),
            subuidname: Default::default(),
            subgidname: Default::default(),
            sysctl: Default::default(),
            systemd: Default::default(),
            tmpfs: Default::default(),
            tty: Default::default(),
            uidmap: Default::default(),
            ulimit: Default::default(),
            user: Default::default(),
            userns: Default::default(),
            uts: Default::default(),
            mount: Default::default(),
            volume: Default::default(),
            volumesFrom: Default::default(),
            workDir: Default::default(),
        };

        let reply = self.client.create_container(create_container).call()?;
        let container = ContainerId(reply.container);

        Ok(container)
    }

    fn start_container(&mut self, name: &str) -> Result<ContainerId> {
        let reply = self.client.start_container(name.to_owned()).call()?;
        let container = ContainerId(reply.container);

        Ok(container)
    }

    fn stop_container(&mut self, name: &str, timeout: u32) -> Result<ContainerId> {
        let reply = self
            .client
            .stop_container(name.to_owned(), timeout as i64)
            .call()?;
        let container = ContainerId(reply.container);

        Ok(container)
    }

    fn remove_container(&mut self, name: &str, remove_volumes: bool) -> Result<ContainerId> {
        let reply = self
            .client
            .remove_container(name.to_owned(), false, remove_volumes)
            .call()?;
        let container = ContainerId(reply.container);

        Ok(container)
    }
}
