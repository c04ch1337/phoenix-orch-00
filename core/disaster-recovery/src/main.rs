use anyhow::Result;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use k8s_openapi::api::core::v1::Secret;
use kube::{
    api::{Api, ListParams, Meta, PostParams},
    Client,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{fs, time::sleep};
use tracing::{error, info, warn};

#[derive(Debug, Serialize, Deserialize)]
struct BackupConfig {
    schedule: BackupSchedule,
    retention: RetentionPolicy,
    storage: StorageConfig,
    targets: Vec<BackupTarget>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BackupSchedule {
    redis_interval_minutes: u32,
    config_interval_minutes: u32,
    state_interval_minutes: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct RetentionPolicy {
    keep_last: u32,
    keep_daily: u32,
    keep_weekly: u32,
    keep_monthly: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct StorageConfig {
    s3_bucket: String,
    s3_prefix: String,
    region: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BackupTarget {
    name: String,
    kind: BackupKind,
    namespace: String,
}

#[derive(Debug, Serialize, Deserialize)]
enum BackupKind {
    Redis,
    Config,
    State,
}

struct DisasterRecoveryController {
    client: Client,
    s3_client: aws_sdk_s3::Client,
    config: Arc<BackupConfig>,
}

impl DisasterRecoveryController {
    async fn new(config_path: &str) -> Result<Self> {
        let client = Client::try_default().await?;
        
        let config = aws_config::load_from_env().await;
        let s3_client = aws_sdk_s3::Client::new(&config);
        
        let config = Arc::new(
            serde_yaml::from_str(&fs::read_to_string(config_path).await?)?,
        );

        Ok(Self {
            client,
            s3_client,
            config,
        })
    }

    async fn run_backup_jobs(&self) -> Result<()> {
        let redis_interval = Duration::from_secs(
            (self.config.schedule.redis_interval_minutes * 60) as u64,
        );
        let config_interval = Duration::from_secs(
            (self.config.schedule.config_interval_minutes * 60) as u64,
        );
        let state_interval = Duration::from_secs(
            (self.config.schedule.state_interval_minutes * 60) as u64,
        );

        loop {
            for target in &self.config.targets {
                match target.kind {
                    BackupKind::Redis => {
                        if let Err(e) = self.backup_redis(target).await {
                            error!("Redis backup failed: {}", e);
                        }
                        sleep(redis_interval).await;
                    }
                    BackupKind::Config => {
                        if let Err(e) = self.backup_config(target).await {
                            error!("Config backup failed: {}", e);
                        }
                        sleep(config_interval).await;
                    }
                    BackupKind::State => {
                        if let Err(e) = self.backup_state(target).await {
                            error!("State backup failed: {}", e);
                        }
                        sleep(state_interval).await;
                    }
                }
            }
        }
    }

    async fn backup_redis(&self, target: &BackupTarget) -> Result<()> {
        info!("Starting Redis backup for {}", target.name);

        // Execute Redis SAVE command
        let pod_name = format!("{}-0", target.name);
        let output = self.execute_in_pod(
            &pod_name,
            &target.namespace,
            "redis-cli",
            &["SAVE"],
        ).await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Redis SAVE command failed"));
        }

        // Copy dump.rdb from pod
        let dump_path = PathBuf::from("/data/dump.rdb");
        let local_path = PathBuf::from("/tmp").join(format!(
            "redis-backup-{}.rdb",
            Utc::now().format("%Y%m%d-%H%M%S"),
        ));

        self.copy_from_pod(
            &pod_name,
            &target.namespace,
            &dump_path,
            &local_path,
        ).await?;

        // Upload to S3
        let key = format!(
            "{}/redis/{}/{}.rdb",
            self.config.storage.s3_prefix,
            target.name,
            Utc::now().format("%Y%m%d-%H%M%S"),
        );

        self.upload_to_s3(&local_path, &key).await?;

        // Cleanup local file
        fs::remove_file(local_path).await?;

        info!("Redis backup completed successfully");
        Ok(())
    }

    async fn backup_config(&self, target: &BackupTarget) -> Result<()> {
        info!("Starting config backup for {}", target.name);

        // Get all ConfigMaps and Secrets
        let configs = self.get_configs(&target.namespace).await?;
        let secrets = self.get_secrets(&target.namespace).await?;

        // Create backup archive
        let backup_path = PathBuf::from("/tmp").join(format!(
            "config-backup-{}.tar.gz",
            Utc::now().format("%Y%m%d-%H%M%S"),
        ));

        let mut archive = tar::Builder::new(
            fs::File::create(&backup_path).await?,
        );

        // Add configs and secrets to archive
        archive.append_serialize("configs.json", &configs)?;
        archive.append_serialize("secrets.json", &secrets)?;
        archive.finish()?;

        // Upload to S3
        let key = format!(
            "{}/config/{}/{}.tar.gz",
            self.config.storage.s3_prefix,
            target.name,
            Utc::now().format("%Y%m%d-%H%M%S"),
        );

        self.upload_to_s3(&backup_path, &key).await?;

        // Cleanup
        fs::remove_file(backup_path).await?;

        info!("Config backup completed successfully");
        Ok(())
    }

    async fn backup_state(&self, target: &BackupTarget) -> Result<()> {
        info!("Starting state backup for {}", target.name);

        // Get PVC name
        let pvc_name = format!("{}-data", target.name);

        // Create snapshot
        let snapshot_name = format!(
            "{}-snapshot-{}",
            target.name,
            Utc::now().format("%Y%m%d-%H%M%S"),
        );

        self.create_volume_snapshot(
            &pvc_name,
            &snapshot_name,
            &target.namespace,
        ).await?;

        info!("State backup completed successfully");
        Ok(())
    }

    async fn execute_in_pod(
        &self,
        pod_name: &str,
        namespace: &str,
        command: &str,
        args: &[&str],
    ) -> Result<std::process::Output> {
        let pods: Api<k8s_openapi::api::core::v1::Pod> = Api::namespaced(
            self.client.clone(),
            namespace,
        );

        let output = pods
            .exec(
                pod_name,
                vec![command],
                None,
            )
            .await?;

        Ok(output)
    }

    async fn copy_from_pod(
        &self,
        pod_name: &str,
        namespace: &str,
        source: &PathBuf,
        destination: &PathBuf,
    ) -> Result<()> {
        let pods: Api<k8s_openapi::api::core::v1::Pod> = Api::namespaced(
            self.client.clone(),
            namespace,
        );

        pods.copy(pod_name, source, destination).await?;
        Ok(())
    }

    async fn upload_to_s3(&self, local_path: &PathBuf, key: &str) -> Result<()> {
        let body = aws_sdk_s3::ByteStream::from_path(local_path).await?;

        self.s3_client
            .put_object()
            .bucket(&self.config.storage.s3_bucket)
            .key(key)
            .body(body)
            .send()
            .await?;

        Ok(())
    }

    async fn get_configs(
        &self,
        namespace: &str,
    ) -> Result<Vec<k8s_openapi::api::core::v1::ConfigMap>> {
        let configs: Api<k8s_openapi::api::core::v1::ConfigMap> = Api::namespaced(
            self.client.clone(),
            namespace,
        );

        let lp = ListParams::default();
        let config_list = configs.list(&lp).await?;

        Ok(config_list.items)
    }

    async fn get_secrets(&self, namespace: &str) -> Result<Vec<Secret>> {
        let secrets: Api<Secret> = Api::namespaced(
            self.client.clone(),
            namespace,
        );

        let lp = ListParams::default();
        let secret_list = secrets.list(&lp).await?;

        Ok(secret_list.items)
    }

    async fn create_volume_snapshot(
        &self,
        pvc_name: &str,
        snapshot_name: &str,
        namespace: &str,
    ) -> Result<()> {
        let snapshots: Api<k8s_openapi::api::snapshot::v1::VolumeSnapshot> = Api::namespaced(
            self.client.clone(),
            namespace,
        );

        let snapshot = k8s_openapi::api::snapshot::v1::VolumeSnapshot {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(snapshot_name.to_string()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            spec: k8s_openapi::api::snapshot::v1::VolumeSnapshotSpec {
                source: k8s_openapi::api::snapshot::v1::VolumeSnapshotSource {
                    persistent_volume_claim_name: Some(pvc_name.to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        snapshots.create(&PostParams::default(), &snapshot).await?;
        Ok(())
    }

    async fn cleanup_old_backups(&self) -> Result<()> {
        // List all backups
        let objects = self.s3_client
            .list_objects_v2()
            .bucket(&self.config.storage.s3_bucket)
            .prefix(&self.config.storage.s3_prefix)
            .send()
            .await?;

        // Group by backup type and sort by date
        let mut redis_backups = Vec::new();
        let mut config_backups = Vec::new();

        if let Some(contents) = objects.contents() {
            for object in contents {
                if let Some(key) = &object.key {
                    if key.contains("/redis/") {
                        redis_backups.push(object);
                    } else if key.contains("/config/") {
                        config_backups.push(object);
                    }
                }
            }
        }

        // Apply retention policy
        self.apply_retention_policy(&redis_backups).await?;
        self.apply_retention_policy(&config_backups).await?;

        Ok(())
    }

    async fn apply_retention_policy(
        &self,
        backups: &[aws_sdk_s3::model::Object],
    ) -> Result<()> {
        let retention = &self.config.retention;
        let mut to_delete = Vec::new();

        // Sort backups by date (newest first)
        let mut sorted_backups = backups.to_vec();
        sorted_backups.sort_by(|a, b| b.last_modified().cmp(&a.last_modified()));

        // Keep last N backups
        let keep_last = sorted_backups.iter()
            .take(retention.keep_last as usize)
            .collect::<Vec<_>>();

        // Keep daily backups
        let daily = sorted_backups.iter()
            .filter(|b| {
                if let Some(date) = b.last_modified() {
                    // Keep if it's the first backup of the day
                    sorted_backups.iter()
                        .filter(|other| {
                            other.last_modified().map(|d| d.date() == date.date()).unwrap_or(false)
                        })
                        .next()
                        .map(|first| first.key() == b.key())
                        .unwrap_or(false)
                } else {
                    false
                }
            })
            .take(retention.keep_daily as usize)
            .collect::<Vec<_>>();

        // Mark others for deletion
        for backup in sorted_backups {
            if !keep_last.contains(&&backup) && !daily.contains(&&backup) {
                if let Some(key) = backup.key() {
                    to_delete.push(key.to_string());
                }
            }
        }

        // Delete marked backups
        for key in to_delete {
            self.s3_client
                .delete_object()
                .bucket(&self.config.storage.s3_bucket)
                .key(key)
                .send()
                .await?;
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "/etc/disaster-recovery/config.yaml".to_string());

    let controller = DisasterRecoveryController::new(&config_path).await?;

    // Run backup jobs and cleanup in parallel
    tokio::select! {
        res = controller.run_backup_jobs() => {
            if let Err(e) = res {
                error!("Backup jobs failed: {}", e);
            }
        }
        res = async {
            loop {
                if let Err(e) = controller.cleanup_old_backups().await {
                    error!("Cleanup failed: {}", e);
                }
                sleep(Duration::from_secs(3600)).await;
            }
        } => {
            if let Err(e) = res {
                error!("Cleanup task failed: {}", e);
            }
        }
    }

    Ok(())
}