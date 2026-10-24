//! Background jobs shown in the status bar (indexing, builds, agent runs...).
//! Model only: whoever starts work registers a job and finishes it.

use std::collections::BTreeMap;

use crate::id::define_id;

define_id!(
    /// One background job.
    JobId
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobStatus {
    Running,
    Succeeded,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct Job {
    id: JobId,
    pub title: String,
    status: JobStatus,
}

impl Job {
    pub fn id(&self) -> JobId {
        self.id
    }

    pub fn status(&self) -> &JobStatus {
        &self.status
    }

    pub fn is_running(&self) -> bool {
        self.status == JobStatus::Running
    }
}

/// All jobs of a window, in start order. Finished jobs stay until `clear_finished`.
#[derive(Default)]
pub struct JobStore {
    next_id: u64,
    jobs: BTreeMap<JobId, Job>,
}

impl JobStore {
    pub fn start(&mut self, title: impl Into<String>) -> JobId {
        self.next_id += 1;
        let id = JobId(self.next_id);
        self.jobs.insert(
            id,
            Job {
                id,
                title: title.into(),
                status: JobStatus::Running,
            },
        );
        id
    }

    /// Changes a job's title (progress: "Downloading x: 4 / 17 MB").
    pub fn set_title(&mut self, id: JobId, title: impl Into<String>) {
        if let Some(job) = self.jobs.get_mut(&id) {
            job.title = title.into();
        }
    }

    /// Marks a job done; `Err(message)` for a failure. Unknown ids are ignored.
    pub fn finish(&mut self, id: JobId, result: Result<(), String>) {
        if let Some(job) = self.jobs.get_mut(&id) {
            job.status = match result {
                Ok(()) => JobStatus::Succeeded,
                Err(message) => JobStatus::Failed(message),
            };
        }
    }

    pub fn get(&self, id: JobId) -> Option<&Job> {
        self.jobs.get(&id)
    }

    pub fn list(&self) -> impl Iterator<Item = &Job> {
        self.jobs.values()
    }

    pub fn running(&self) -> impl Iterator<Item = &Job> {
        self.jobs.values().filter(|job| job.is_running())
    }

    pub fn clear_finished(&mut self) {
        self.jobs.retain(|_, job| job.is_running());
    }
}
