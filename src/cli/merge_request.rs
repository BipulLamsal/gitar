use std::option::Option;

use clap::{Parser, ValueEnum};

use crate::{
    cmds::merge_request::{
        CommentMergeRequestCliArgs, MergeRequestCliArgs, MergeRequestGetCliArgs,
        MergeRequestListCliArgs,
    },
    remote::MergeRequestState,
};

use super::common::{GetArgs, ListArgs};

#[derive(Parser)]
pub struct MergeRequestCommand {
    #[clap(subcommand)]
    subcommand: MergeRequestSubcommand,
}

#[derive(Parser)]
enum MergeRequestSubcommand {
    #[clap(about = "Creates a merge request", visible_alias = "cr")]
    Create(CreateMergeRequest),
    #[clap(about = "Approve a merge request", visible_alias = "ap")]
    Approve(ApproveMergeRequest),
    #[clap(about = "Merge a merge request")]
    Merge(MergeMergeRequest),
    #[clap(about = "Git checkout a merge request branch for review")]
    Checkout(CheckoutMergeRequest),
    #[clap(about = "Comment on a merge request")]
    Comment(CommentMergeRequest),
    #[clap(about = "Close a merge request")]
    Close(CloseMergeRequest),
    /// Get a merge request
    Get(GetMergeRequest),
    #[clap(about = "List merge requests", visible_alias = "ls")]
    List(ListMergeRequest),
}

#[derive(Parser)]
struct GetMergeRequest {
    /// Id of the merge request
    #[clap()]
    id: i64,
    #[clap(flatten)]
    get_args: GetArgs,
}

#[derive(Parser)]
struct CommentMergeRequest {
    /// Id of the merge request
    #[clap(long)]
    pub id: i64,
    /// Comment to add to the merge request
    #[clap(group = "comment_msg")]
    pub comment: Option<String>,
    /// Gather comment from the specified file. If "-" is provided, read from STDIN
    #[clap(long, value_name = "FILE", group = "comment_msg")]
    pub comment_from_file: Option<String>,
}

#[derive(Parser)]
struct CreateMergeRequest {
    /// Title of the merge request
    #[clap(long, group = "title_msg")]
    pub title: Option<String>,
    /// Gather title and description from the specified commit message
    #[clap(long, group = "title_msg", value_name = "SHA")]
    pub title_from_commit: Option<String>,
    /// Description of the merge request
    #[clap(long)]
    pub description: Option<String>,
    /// Gather merge request description from the specified file. If "-" is
    /// provided, read from STDIN
    #[clap(long, value_name = "FILE")]
    pub description_from_file: Option<String>,
    /// Accept the default title, description, and target branch
    #[clap(long, short)]
    pub auto: bool,
    /// Target branch of the merge request instead of default project's upstream branch
    #[clap(long)]
    pub target_branch: Option<String>,
    /// Refresh the cache
    #[clap(long, short)]
    pub refresh: bool,
    /// Automatically open the browser after creating the merge request
    #[clap(long, short)]
    pub browse: bool,
    /// Open the merge request automatically without prompting for confirmation
    #[clap(long, short)]
    pub yes: bool,
    /// Adds and commits all changes before creating the merge request
    #[clap(long)]
    pub commit: Option<String>,
    /// Set up the merge request as draft
    #[clap(long, visible_alias = "wip")]
    pub draft: bool,
}

#[derive(ValueEnum, Clone, PartialEq, Debug)]
pub enum MergeRequestStateStateCli {
    Opened,
    Closed,
    Merged,
}

impl From<MergeRequestStateStateCli> for MergeRequestState {
    fn from(state: MergeRequestStateStateCli) -> Self {
        match state {
            MergeRequestStateStateCli::Opened => MergeRequestState::Opened,
            MergeRequestStateStateCli::Closed => MergeRequestState::Closed,
            MergeRequestStateStateCli::Merged => MergeRequestState::Merged,
        }
    }
}

#[derive(Parser)]
pub struct ListMergeRequest {
    #[clap()]
    pub state: MergeRequestStateStateCli,
    #[command(flatten)]
    pub list_args: ListArgs,
}

#[derive(Parser)]
struct MergeMergeRequest {
    /// Id of the merge request
    #[clap()]
    pub id: i64,
}

#[derive(Parser)]
struct CheckoutMergeRequest {
    /// Id of the merge request
    #[clap()]
    pub id: i64,
}

#[derive(Parser)]
struct CloseMergeRequest {
    /// Id of the merge request
    #[clap()]
    pub id: i64,
}

#[derive(Parser)]
struct ApproveMergeRequest {
    /// Id of the merge request
    #[clap()]
    pub id: i64,
}

impl From<ListMergeRequest> for MergeRequestOptions {
    fn from(options: ListMergeRequest) -> Self {
        MergeRequestOptions::List(MergeRequestListCliArgs::new(
            options.state.into(),
            options.list_args.into(),
        ))
    }
}

impl From<MergeMergeRequest> for MergeRequestOptions {
    fn from(options: MergeMergeRequest) -> Self {
        MergeRequestOptions::Merge { id: options.id }
    }
}

impl From<CheckoutMergeRequest> for MergeRequestOptions {
    fn from(options: CheckoutMergeRequest) -> Self {
        MergeRequestOptions::Checkout { id: options.id }
    }
}

impl From<CloseMergeRequest> for MergeRequestOptions {
    fn from(options: CloseMergeRequest) -> Self {
        MergeRequestOptions::Close { id: options.id }
    }
}

impl From<ApproveMergeRequest> for MergeRequestOptions {
    fn from(options: ApproveMergeRequest) -> Self {
        MergeRequestOptions::Approve { id: options.id }
    }
}

impl From<MergeRequestCommand> for MergeRequestOptions {
    fn from(options: MergeRequestCommand) -> Self {
        match options.subcommand {
            MergeRequestSubcommand::Create(options) => options.into(),
            MergeRequestSubcommand::List(options) => options.into(),
            MergeRequestSubcommand::Merge(options) => options.into(),
            MergeRequestSubcommand::Checkout(options) => options.into(),
            MergeRequestSubcommand::Close(options) => options.into(),
            MergeRequestSubcommand::Comment(options) => options.into(),
            MergeRequestSubcommand::Get(options) => options.into(),
            MergeRequestSubcommand::Approve(options) => options.into(),
        }
    }
}

impl From<CreateMergeRequest> for MergeRequestOptions {
    fn from(options: CreateMergeRequest) -> Self {
        MergeRequestOptions::Create(
            MergeRequestCliArgs::builder()
                .title(options.title)
                .title_from_commit(options.title_from_commit)
                .description(options.description)
                .description_from_file(options.description_from_file)
                .target_branch(options.target_branch)
                .auto(options.auto)
                .refresh_cache(options.refresh)
                .open_browser(options.browse)
                .accept_summary(options.yes)
                .commit(options.commit)
                .draft(options.draft)
                .build()
                .unwrap(),
        )
    }
}

impl From<CommentMergeRequest> for MergeRequestOptions {
    fn from(options: CommentMergeRequest) -> Self {
        MergeRequestOptions::Comment(
            CommentMergeRequestCliArgs::builder()
                .id(options.id)
                .comment(options.comment)
                .comment_from_file(options.comment_from_file)
                .build()
                .unwrap(),
        )
    }
}

impl From<GetMergeRequest> for MergeRequestOptions {
    fn from(options: GetMergeRequest) -> Self {
        MergeRequestOptions::Get(
            MergeRequestGetCliArgs::builder()
                .id(options.id)
                .get_args(options.get_args.into())
                .build()
                .unwrap(),
        )
    }
}

pub enum MergeRequestOptions {
    Create(MergeRequestCliArgs),
    Get(MergeRequestGetCliArgs),
    List(MergeRequestListCliArgs),
    Comment(CommentMergeRequestCliArgs),
    Approve { id: i64 },
    Merge { id: i64 },
    Checkout { id: i64 },
    Close { id: i64 },
}

#[cfg(test)]
mod test {
    use crate::cli::{Args, Command};

    use super::*;

    #[test]
    fn test_list_merge_requests_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "list", "opened"]);
        let list_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::List(options),
            }) => {
                assert_eq!(options.state, MergeRequestStateStateCli::Opened);
                options
            }
            _ => panic!("Expected MergeRequestCommand::List"),
        };

        let options: MergeRequestOptions = list_merge_request.into();
        match options {
            MergeRequestOptions::List(args) => {
                assert_eq!(args.state, MergeRequestState::Opened);
            }
            _ => panic!("Expected MergeRequestOptions::List"),
        }
    }

    #[test]
    fn test_merge_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "merge", "123"]);
        let merge_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Merge(options),
            }) => {
                assert_eq!(options.id, 123);
                options
            }
            _ => panic!("Expected MergeRequestCommand::Merge"),
        };

        let options: MergeRequestOptions = merge_merge_request.into();
        match options {
            MergeRequestOptions::Merge { id } => {
                assert_eq!(id, 123);
            }
            _ => panic!("Expected MergeRequestOptions::Merge"),
        }
    }

    #[test]
    fn test_checkout_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "checkout", "123"]);
        let checkout_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Checkout(options),
            }) => {
                assert_eq!(options.id, 123);
                options
            }
            _ => panic!("Expected MergeRequestCommand::Checkout"),
        };

        let options: MergeRequestOptions = checkout_merge_request.into();
        match options {
            MergeRequestOptions::Checkout { id } => {
                assert_eq!(id, 123);
            }
            _ => panic!("Expected MergeRequestOptions::Checkout"),
        }
    }

    #[test]
    fn test_close_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "close", "123"]);
        let close_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Close(options),
            }) => {
                assert_eq!(options.id, 123);
                options
            }
            _ => panic!("Expected MergeRequestCommand::Close"),
        };

        let options: MergeRequestOptions = close_merge_request.into();
        match options {
            MergeRequestOptions::Close { id } => {
                assert_eq!(id, 123);
            }
            _ => panic!("Expected MergeRequestOptions::Close"),
        }
    }

    #[test]
    fn test_comment_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "comment", "--id", "123", "LGTM"]);
        let comment_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Comment(options),
            }) => {
                assert_eq!(options.id, 123);
                assert_eq!(options.comment, Some("LGTM".to_string()));
                options
            }
            _ => panic!("Expected MergeRequestCommand::Comment"),
        };

        let options: MergeRequestOptions = comment_merge_request.into();
        match options {
            MergeRequestOptions::Comment(args) => {
                assert_eq!(args.id, 123);
                assert_eq!(args.comment, Some("LGTM".to_string()));
            }
            _ => panic!("Expected MergeRequestOptions::Comment"),
        }
    }

    #[test]
    fn test_create_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "create", "--auto", "-y", "--browse"]);
        let create_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Create(options),
            }) => {
                assert!(options.auto);
                assert!(options.yes);
                assert!(options.browse);
                options
            }
            _ => panic!("Expected MergeRequestCommand::Create"),
        };

        let options: MergeRequestOptions = create_merge_request.into();
        match options {
            MergeRequestOptions::Create(args) => {
                assert!(args.auto);
                assert!(args.accept_summary);
                assert!(args.open_browser);
            }
            _ => panic!("Expected MergeRequestOptions::Create"),
        }
    }

    #[test]
    fn test_get_merge_request_details_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "get", "123"]);
        let get_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Get(options),
            }) => {
                assert_eq!(options.id, 123);
                options
            }
            _ => panic!("Expected MergeRequestCommand::Get"),
        };

        let options: MergeRequestOptions = get_merge_request.into();
        match options {
            MergeRequestOptions::Get(args) => {
                assert_eq!(args.id, 123);
            }
            _ => panic!("Expected MergeRequestOptions::Get"),
        }
    }

    #[test]
    fn test_wip_alias_as_draft() {
        let args = Args::parse_from(vec!["gr", "mr", "create", "--auto", "--wip"]);
        let create_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Create(options),
            }) => {
                assert!(options.draft);
                options
            }
            _ => panic!("Expected MergeRequestCommand::Create"),
        };

        let options: MergeRequestOptions = create_merge_request.into();
        match options {
            MergeRequestOptions::Create(args) => {
                assert!(args.draft);
            }
            _ => panic!("Expected MergeRequestOptions::Create"),
        }
    }
}
