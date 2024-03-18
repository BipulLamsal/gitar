use crate::api_traits::{Cicd, CicdRunner, Timestamp};
use crate::cli::cicd::{PipelineOptions, RunnerOptions};
use crate::config::Config;
use crate::display::{Column, DisplayBody, Format};
use crate::remote::{ListBodyArgs, ListRemoteCliArgs};
use crate::{display, remote, Result};
use std::fmt::Display;
use std::io::Write;
use std::sync::Arc;

use super::common::{num_cicd_pages, process_num_pages};

#[derive(Builder, Clone, Debug)]
pub struct Pipeline {
    pub status: String,
    web_url: String,
    branch: String,
    sha: String,
    created_at: String,
    updated_at: String,
    duration: u64,
}

impl Pipeline {
    pub fn builder() -> PipelineBuilder {
        PipelineBuilder::default()
    }
}

impl Timestamp for Pipeline {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

impl From<Pipeline> for DisplayBody {
    fn from(p: Pipeline) -> DisplayBody {
        DisplayBody {
            columns: vec![
                Column::new("URL", p.web_url),
                Column::new("Branch", p.branch),
                Column::new("SHA", p.sha),
                Column::new("Created at", p.created_at),
                Column::new("Updated at", p.updated_at),
                Column::new("Duration", p.duration.to_string()),
                Column::new("Status", p.status),
            ],
        }
    }
}

#[derive(Builder, Clone)]
pub struct PipelineBodyArgs {
    pub from_to_page: Option<ListBodyArgs>,
}

impl PipelineBodyArgs {
    pub fn builder() -> PipelineBodyArgsBuilder {
        PipelineBodyArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct Runner {
    pub id: i64,
    pub active: bool,
    pub description: String,
    pub ip_address: String,
    pub name: String,
    pub online: bool,
    pub paused: bool,
    pub is_shared: bool,
    pub runner_type: String,
    pub status: String,
}

impl Runner {
    pub fn builder() -> RunnerBuilder {
        RunnerBuilder::default()
    }
}

impl From<Runner> for DisplayBody {
    fn from(r: Runner) -> DisplayBody {
        DisplayBody {
            columns: vec![
                Column::new("ID", r.id.to_string()),
                Column::new("Active", r.active.to_string()),
                Column::new("Description", r.description),
                Column::new("IP Address", r.ip_address),
                Column::new("Name", r.name),
                Column::new("Paused", r.paused.to_string()),
                Column::new("Shared", r.is_shared.to_string()),
                Column::new("Type", r.runner_type),
                Column::new("Online", r.online.to_string()),
                Column::new("Status", r.status.to_string()),
            ],
        }
    }
}

impl Timestamp for Runner {
    fn created_at(&self) -> String {
        // There is no created_at field for runners, set it to UNIX epoch
        "1970-01-01T00:00:00Z".to_string()
    }
}

#[derive(Builder, Clone)]
pub struct RunnerListCliArgs {
    pub status: RunnerStatus,
    #[builder(default)]
    pub tags: Option<String>,
    pub list_args: ListRemoteCliArgs,
}

impl RunnerListCliArgs {
    pub fn builder() -> RunnerListCliArgsBuilder {
        RunnerListCliArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct RunnerListBodyArgs {
    pub list_args: Option<ListBodyArgs>,
    pub status: RunnerStatus,
    #[builder(default)]
    pub tags: Option<String>,
}

impl RunnerListBodyArgs {
    pub fn builder() -> RunnerListBodyArgsBuilder {
        RunnerListBodyArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct RunnerMetadataCliArgs {
    pub id: i64,
    pub refresh_cache: bool,
    pub no_headers: bool,
    #[builder(default)]
    pub format: Format,
}

impl RunnerMetadataCliArgs {
    pub fn builder() -> RunnerMetadataCliArgsBuilder {
        RunnerMetadataCliArgsBuilder::default()
    }
}

#[derive(Clone, Copy)]
pub enum RunnerStatus {
    Online,
    Offline,
    Stale,
    NeverContacted,
}

impl Display for RunnerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunnerStatus::Online => write!(f, "online"),
            RunnerStatus::Offline => write!(f, "offline"),
            RunnerStatus::Stale => write!(f, "stale"),
            RunnerStatus::NeverContacted => write!(f, "never_contacted"),
        }
    }
}

pub fn execute(
    options: PipelineOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        PipelineOptions::List(cli_args) => {
            let remote = remote::get_cicd(domain, path, config, cli_args.refresh_cache)?;
            if cli_args.num_pages {
                return num_cicd_pages(remote, std::io::stdout());
            }
            let from_to_args = remote::validate_from_to_page(&cli_args)?;
            let body_args = PipelineBodyArgs::builder()
                .from_to_page(from_to_args)
                .build()?;
            list_pipelines(remote, body_args, cli_args, std::io::stdout())
        }
        PipelineOptions::Runners(options) => match options {
            RunnerOptions::List(cli_args) => {
                let remote = remote::get_cicd_runner(
                    domain,
                    path,
                    config,
                    cli_args.list_args.refresh_cache,
                )?;
                let from_to_args = remote::validate_from_to_page(&cli_args.list_args)?;
                let tags = cli_args.tags.clone();
                let body_args = RunnerListBodyArgs::builder()
                    .list_args(from_to_args)
                    .status(cli_args.status)
                    .tags(tags)
                    .build()?;
                if cli_args.list_args.num_pages {
                    return process_num_pages(remote.num_pages(body_args), std::io::stdout());
                }

                list_runners(remote, body_args, cli_args, std::io::stdout())
            }
            RunnerOptions::Get(_cli_args) => {
                todo!();
            }
        },
    }
}

fn list_runners<W: Write>(
    remote: Arc<dyn CicdRunner>,
    body_args: RunnerListBodyArgs,
    cli_args: RunnerListCliArgs,
    mut writer: W,
) -> Result<()> {
    let runners = remote.list(body_args)?;
    if runners.is_empty() {
        writer.write_all(b"No runners found.\n")?;
        return Ok(());
    }
    display::print(
        &mut writer,
        runners,
        cli_args.list_args.no_headers,
        &cli_args.list_args.format,
    )?;
    Ok(())
}

fn list_pipelines<W: Write>(
    remote: Arc<dyn Cicd>,
    body_args: PipelineBodyArgs,
    cli_args: ListRemoteCliArgs,
    mut writer: W,
) -> Result<()> {
    let pipelines = remote.list(body_args)?;
    if pipelines.is_empty() {
        writer.write_all(b"No pipelines found.\n")?;
        return Ok(());
    }
    display::print(
        &mut writer,
        pipelines,
        cli_args.no_headers,
        &cli_args.format,
    )?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::error;

    #[derive(Clone, Builder)]
    struct PipelineListMock {
        #[builder(default = "vec![]")]
        pipelines: Vec<Pipeline>,
        #[builder(default = "false")]
        error: bool,
        #[builder(setter(into, strip_option), default)]
        num_pages: Option<u32>,
    }

    impl PipelineListMock {
        pub fn builder() -> PipelineListMockBuilder {
            PipelineListMockBuilder::default()
        }
    }

    impl Cicd for PipelineListMock {
        fn list(&self, _args: PipelineBodyArgs) -> Result<Vec<Pipeline>> {
            if self.error {
                return Err(error::gen("Error"));
            }
            let pp = self.pipelines.clone();
            Ok(pp)
        }

        fn get_pipeline(&self, _id: i64) -> Result<Pipeline> {
            let pp = self.pipelines.clone();
            Ok(pp[0].clone())
        }

        fn num_pages(&self) -> Result<Option<u32>> {
            if self.error {
                return Err(error::gen("Error"));
            }
            return Ok(self.num_pages);
        }
    }

    #[test]
    fn test_list_pipelines() {
        let pp_remote = PipelineListMock::builder()
            .pipelines(vec![
                Pipeline::builder()
                    .status("success".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/123".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .updated_at("2020-01-01T00:01:00Z".to_string())
                    .duration(60)
                    .build()
                    .unwrap(),
                Pipeline::builder()
                    .status("failed".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/456".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .updated_at("2020-01-01T00:01:01Z".to_string())
                    .duration(61)
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap();
        let mut buf = Vec::new();
        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().build().unwrap();
        list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "URL | Branch | SHA | Created at | Updated at | Duration | Status\n\
             https://gitlab.com/owner/repo/-/pipelines/123 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | 2020-01-01T00:01:00Z | 60 | success\n\
             https://gitlab.com/owner/repo/-/pipelines/456 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | 2020-01-01T00:01:01Z | 61 | failed\n")
    }

    #[test]
    fn test_list_pipelines_empty() {
        let pp_remote = PipelineListMock::builder().build().unwrap();
        let mut buf = Vec::new();

        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().build().unwrap();
        list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!("No pipelines found.\n", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_list_pipelines_error() {
        let pp_remote = PipelineListMock::builder().error(true).build().unwrap();
        let mut buf = Vec::new();
        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().build().unwrap();
        assert!(list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).is_err());
    }

    #[test]
    fn test_list_number_of_pipelines_pages() {
        let pp_remote = PipelineListMock::builder()
            .num_pages(3 as u32)
            .build()
            .unwrap();
        let mut buf = Vec::new();
        num_cicd_pages(Arc::new(pp_remote), &mut buf).unwrap();
        assert_eq!("3\n", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_no_pages_available() {
        let pp_remote = PipelineListMock::builder().build().unwrap();
        let mut buf = Vec::new();
        num_cicd_pages(Arc::new(pp_remote), &mut buf).unwrap();
        assert_eq!(
            "Number of pages not available.\n",
            String::from_utf8(buf).unwrap(),
        )
    }

    #[test]
    fn test_number_of_pages_error() {
        let pp_remote = PipelineListMock::builder().error(true).build().unwrap();
        let mut buf = Vec::new();
        assert!(num_cicd_pages(Arc::new(pp_remote), &mut buf).is_err());
    }

    #[test]
    fn test_list_pipelines_no_headers() {
        let pp_remote = PipelineListMock::builder()
            .pipelines(vec![
                Pipeline::builder()
                    .status("success".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/123".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .updated_at("2020-01-01T00:01:00Z".to_string())
                    .duration(60)
                    .build()
                    .unwrap(),
                Pipeline::builder()
                    .status("failed".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/456".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .updated_at("2020-01-01T00:01:00Z".to_string())
                    .duration(60)
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap();
        let mut buf = Vec::new();
        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder()
            .no_headers(true)
            .build()
            .unwrap();
        list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            "https://gitlab.com/owner/repo/-/pipelines/123 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | 2020-01-01T00:01:00Z | 60 | success\n\
             https://gitlab.com/owner/repo/-/pipelines/456 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | 2020-01-01T00:01:00Z | 60 | failed\n",
            String::from_utf8(buf).unwrap(),
        )
    }

    #[derive(Builder, Clone)]
    struct RunnerMock {
        runners: Vec<Runner>,
        #[builder(default)]
        error: bool,
    }

    impl RunnerMock {
        pub fn builder() -> RunnerMockBuilder {
            RunnerMockBuilder::default()
        }
    }

    impl CicdRunner for RunnerMock {
        fn list(&self, _args: RunnerListBodyArgs) -> Result<Vec<Runner>> {
            if self.error {
                return Err(error::gen("Error"));
            }
            let rr = self.runners.clone();
            Ok(rr)
        }

        fn get(&self, _id: i64) -> Result<Runner> {
            let rr = self.runners.clone();
            Ok(rr[0].clone())
        }

        fn num_pages(&self, _args: RunnerListBodyArgs) -> Result<Option<u32>> {
            if self.error {
                return Err(error::gen("Error"));
            }
            Ok(None)
        }
    }

    #[test]
    fn test_list_runners() {
        let runners = vec![
            Runner::builder()
                .id(1)
                .active(true)
                .description("Runner 1".to_string())
                .ip_address("10.0.0.1".to_string())
                .name("runner1".to_string())
                .online(true)
                .status("online".to_string())
                .paused(false)
                .is_shared(true)
                .runner_type("shared".to_string())
                .build()
                .unwrap(),
            Runner::builder()
                .id(2)
                .active(true)
                .description("Runner 2".to_string())
                .ip_address("10.0.0.2".to_string())
                .name("runner2".to_string())
                .online(true)
                .status("online".to_string())
                .paused(false)
                .is_shared(true)
                .runner_type("shared".to_string())
                .build()
                .unwrap(),
        ];
        let remote = RunnerMock::builder().runners(runners).build().unwrap();
        let mut buf = Vec::new();
        let body_args = RunnerListBodyArgs::builder()
            .list_args(None)
            .status(RunnerStatus::Online)
            .build()
            .unwrap();
        let cli_args = RunnerListCliArgs::builder()
            .status(RunnerStatus::Online)
            .list_args(ListRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        list_runners(Arc::new(remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            "ID | Active | Description | IP Address | Name | Paused | Shared | Type | Online | Status\n\
             1 | true | Runner 1 | 10.0.0.1 | runner1 | false | true | shared | true | online\n\
             2 | true | Runner 2 | 10.0.0.2 | runner2 | false | true | shared | true | online\n",
            String::from_utf8(buf).unwrap()
        )
    }
}
