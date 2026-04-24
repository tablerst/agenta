use super::*;

impl AgentaService {
    pub async fn service_overview(&self) -> AppResult<ServiceOverview> {
        Ok(ServiceOverview {
            project_count: self.store.project_count().await?,
            task_count: self.store.task_count().await?,
        })
    }
}
