use clap::{Parser, ValueEnum};

use crate::{
    cmds::cicd::{RunnerListCliArgs, RunnerMetadataGetCliArgs, RunnerStatus},
    remote::ListRemoteCliArgs,
};

use super::common::{GetArgs, ListArgs};

#[derive(Parser)]
pub struct PipelineCommand {
    #[clap(subcommand)]
    subcommand: PipelineSubcommand,
}

#[derive(Parser)]
enum PipelineSubcommand {
    #[clap(about = "List pipelines")]
    List(ListArgs),
    #[clap(subcommand, name = "rn", about = "Runner operations")]
    Runners(RunnerSubCommand),
}

#[derive(Parser)]
enum RunnerSubCommand {
    #[clap(about = "List runners")]
    List(ListRunner),
    #[clap(about = "Get runner metadata")]
    Get(RunnerMetadata),
}

#[derive(ValueEnum, Clone, PartialEq, Debug)]
enum RunnerStatusCli {
    Online,
    Offline,
    Stale,
    NeverContacted,
    All,
}

#[derive(Parser)]
struct ListRunner {
    /// Runner status
    #[clap()]
    status: RunnerStatusCli,
    /// Comma separated list of tags
    #[clap(long, value_delimiter = ',', help_heading = "Runner options")]
    tags: Option<Vec<String>>,
    /// List all runners available across all projects. Gitlab admins only.
    #[clap(long, help_heading = "Runner options")]
    all: bool,
    #[command(flatten)]
    list_args: ListArgs,
}

#[derive(Parser)]
struct RunnerMetadata {
    /// Runner ID
    #[clap()]
    id: i64,
    #[clap(flatten)]
    get_args: GetArgs,
}

impl From<PipelineCommand> for PipelineOptions {
    fn from(options: PipelineCommand) -> Self {
        match options.subcommand {
            PipelineSubcommand::List(options) => options.into(),
            PipelineSubcommand::Runners(options) => options.into(),
        }
    }
}

impl From<ListArgs> for PipelineOptions {
    fn from(options: ListArgs) -> Self {
        PipelineOptions::List(options.into())
    }
}

impl From<RunnerSubCommand> for PipelineOptions {
    fn from(options: RunnerSubCommand) -> Self {
        match options {
            RunnerSubCommand::List(options) => PipelineOptions::Runners(options.into()),
            RunnerSubCommand::Get(options) => PipelineOptions::Runners(options.into()),
        }
    }
}

impl From<RunnerStatusCli> for RunnerStatus {
    fn from(status: RunnerStatusCli) -> Self {
        match status {
            RunnerStatusCli::Online => RunnerStatus::Online,
            RunnerStatusCli::Offline => RunnerStatus::Offline,
            RunnerStatusCli::Stale => RunnerStatus::Stale,
            RunnerStatusCli::NeverContacted => RunnerStatus::NeverContacted,
            RunnerStatusCli::All => RunnerStatus::All,
        }
    }
}

impl From<ListRunner> for RunnerOptions {
    fn from(options: ListRunner) -> Self {
        RunnerOptions::List(
            RunnerListCliArgs::builder()
                .status(options.status.into())
                .tags(options.tags.map(|tags| tags.join(",").to_string()))
                .all(options.all)
                .list_args(options.list_args.into())
                .build()
                .unwrap(),
        )
    }
}

impl From<RunnerMetadata> for RunnerOptions {
    fn from(options: RunnerMetadata) -> Self {
        RunnerOptions::Get(
            RunnerMetadataGetCliArgs::builder()
                .id(options.id)
                .get_args(options.get_args.into())
                .build()
                .unwrap(),
        )
    }
}

pub enum PipelineOptions {
    List(ListRemoteCliArgs),
    Runners(RunnerOptions),
}

pub enum RunnerOptions {
    List(RunnerListCliArgs),
    Get(RunnerMetadataGetCliArgs),
}

#[cfg(test)]
mod test {
    use crate::cli::{Args, Command};

    use super::*;

    #[test]
    fn test_pipeline_cli_list() {
        let args = Args::parse_from(vec![
            "gr",
            "pp",
            "list",
            "--from-page",
            "1",
            "--to-page",
            "2",
        ]);
        let list_args = match args.command {
            Command::Pipeline(PipelineCommand {
                subcommand: PipelineSubcommand::List(options),
            }) => {
                assert_eq!(options.from_page, Some(1));
                assert_eq!(options.to_page, Some(2));
                options
            }
            _ => panic!("Expected PipelineCommand"),
        };
        let options: PipelineOptions = list_args.into();
        match options {
            PipelineOptions::List(args) => {
                assert_eq!(args.from_page, Some(1));
                assert_eq!(args.to_page, Some(2));
            }
            _ => panic!("Expected PipelineOptions::List"),
        }
    }

    #[test]
    fn test_pipeline_cli_runners_list() {
        let args = Args::parse_from(vec![
            "gr",
            "pp",
            "rn",
            "list",
            "online",
            "--tags",
            "tag1,tag2",
            "--all",
            "--from-page",
            "1",
            "--to-page",
            "2",
        ]);
        let list_args = match args.command {
            Command::Pipeline(PipelineCommand {
                subcommand: PipelineSubcommand::Runners(RunnerSubCommand::List(options)),
            }) => {
                assert_eq!(options.status, RunnerStatusCli::Online);
                assert_eq!(
                    options.tags,
                    Some(vec!["tag1".to_string(), "tag2".to_string()])
                );
                assert_eq!(options.all, true);
                assert_eq!(options.list_args.from_page, Some(1));
                assert_eq!(options.list_args.to_page, Some(2));
                options
            }
            _ => panic!("Expected PipelineCommand"),
        };
        let options: RunnerOptions = list_args.into();
        match options {
            RunnerOptions::List(args) => {
                assert_eq!(args.status, RunnerStatus::Online);
                assert_eq!(args.tags, Some("tag1,tag2".to_string()));
                assert_eq!(args.all, true);
                assert_eq!(args.list_args.from_page, Some(1));
                assert_eq!(args.list_args.to_page, Some(2));
            }
            _ => panic!("Expected RunnerOptions::List"),
        }
    }

    #[test]
    fn test_get_gitlab_runner_metadata() {
        let args = Args::parse_from(vec!["gr", "pp", "rn", "get", "123"]);
        let list_args = match args.command {
            Command::Pipeline(PipelineCommand {
                subcommand: PipelineSubcommand::Runners(RunnerSubCommand::Get(options)),
            }) => {
                assert_eq!(options.id, 123);
                options
            }
            _ => panic!("Expected PipelineCommand"),
        };
        let options: RunnerOptions = list_args.into();
        match options {
            RunnerOptions::Get(args) => {
                assert_eq!(args.id, 123);
            }
            _ => panic!("Expected RunnerOptions::Get"),
        }
    }
}
