use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub name: String,
    pub schedule: String,
    pub prompt: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    #[serde(default)]
    pub output: Option<CronOutput>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CronOutput {
    File {
        path: String,
    },
    Pr {
        branch: String,
        #[serde(default)]
        title: Option<String>,
    },
    Notification {
        #[serde(default)]
        channel: Option<String>,
    },
}

fn default_enabled() -> bool {
    true
}

impl CronJob {
    pub fn parse_schedule(&self) -> Result<tokio_cron_scheduler::Job, String> {
        let name = self.name.clone();
        match tokio_cron_scheduler::Job::new_async(
            &self.schedule,
            Box::new(move |_id, _scheduler| {
                let name = name.clone();
                Box::pin(async move {
                    tracing::info!(job_name = name, "Cron job triggered");
                })
                    as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
            }),
        ) {
            Ok(job) => Ok(job),
            Err(e) => Err(format!("Invalid cron schedule '{}': {e}", self.schedule)),
        }
    }
}
