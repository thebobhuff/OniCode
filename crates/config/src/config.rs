use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    agent_config::AgentConfig, cron::CronJob, hooks::Hook, settings::Settings, skill::Skill,
};

pub struct OniCodeConfig {
    pub settings: Settings,
    pub agents: HashMap<String, AgentConfig>,
    pub skills: Vec<Skill>,
    pub hooks: Vec<Hook>,
    pub cron_jobs: Vec<CronJob>,
    pub mcp_config: onicode_mcp::McpServerConfig,
    pub config_dir: PathBuf,
    pub workspace_root: PathBuf,
}

impl OniCodeConfig {
    pub async fn load(workspace_root: &std::path::Path) -> Self {
        let config_dir = workspace_root.join(".onicode");
        std::fs::create_dir_all(&config_dir).ok();

        let settings = Settings::load(workspace_root).await;
        let agents = Self::load_agents(&config_dir);
        let skills = Self::load_skills(&config_dir);
        let hooks = Self::load_hooks(&config_dir);
        let cron_jobs = Self::load_cron_jobs(&config_dir);
        let mcp_config = onicode_mcp::McpServerConfig::load_from_dir(workspace_root);

        Self {
            settings,
            agents,
            skills,
            hooks,
            cron_jobs,
            mcp_config,
            config_dir: config_dir.to_path_buf(),
            workspace_root: workspace_root.to_path_buf(),
        }
    }

    pub fn get_agent(&self, name: &str) -> Option<&AgentConfig> {
        self.agents.get(name)
    }

    pub fn get_skill(&self, name: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.name == name)
    }

    pub fn get_hooks_for_event(&self, event: &crate::hooks::HookEvent) -> Vec<&Hook> {
        self.hooks.iter().filter(|h| h.event == *event).collect()
    }

    fn load_agents(config_dir: &std::path::Path) -> HashMap<String, AgentConfig> {
        let agents_dir = config_dir.join("agents");
        let mut agents = HashMap::new();

        if !agents_dir.exists() {
            agents.insert("build".to_string(), AgentConfig::default_build());
            agents.insert("plan".to_string(), AgentConfig::default_plan());
            return agents;
        }

        for entry in std::fs::read_dir(&agents_dir).ok().into_iter().flatten() {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    if let Ok(agent) = AgentConfig::from_file(&path) {
                        agents.insert(agent.name.clone(), agent);
                    }
                }
            }
        }

        if !agents.contains_key("build") {
            agents.insert("build".to_string(), AgentConfig::default_build());
        }
        if !agents.contains_key("plan") {
            agents.insert("plan".to_string(), AgentConfig::default_plan());
        }

        agents
    }

    fn load_skills(config_dir: &std::path::Path) -> Vec<Skill> {
        let skills_dir = config_dir.join("skills");
        let mut skills = Vec::new();

        if !skills_dir.exists() {
            return skills;
        }

        for entry in std::fs::read_dir(&skills_dir).ok().into_iter().flatten() {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    let skill_md = path.join("SKILL.md");
                    if skill_md.exists() {
                        if let Ok(skill) = Skill::from_file(&skill_md) {
                            skills.push(skill);
                        }
                    }
                }
            }
        }

        skills
    }

    fn load_hooks(config_dir: &std::path::Path) -> Vec<Hook> {
        let hooks_file = config_dir.join("hooks.json");
        if hooks_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&hooks_file) {
                if let Ok(hooks) = serde_json::from_str::<Vec<Hook>>(&content) {
                    return hooks;
                }
            }
        }
        Vec::new()
    }

    fn load_cron_jobs(config_dir: &std::path::Path) -> Vec<CronJob> {
        let cron_file = config_dir.join("cron.json");
        if cron_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&cron_file) {
                if let Ok(jobs) = serde_json::from_str::<Vec<CronJob>>(&content) {
                    return jobs;
                }
            }
        }
        Vec::new()
    }
}
