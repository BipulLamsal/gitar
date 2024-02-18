use crate::api_traits::ApiOperation;
use crate::api_traits::Cicd;
use crate::api_traits::RemoteProject;
use crate::cli::BrowseOptions;
use crate::config::ConfigProperties;
use crate::error::GRError;
use crate::http::Headers;
use crate::http::Method::GET;
use crate::io::CmdInfo;
use crate::io::HttpRunner;
use crate::io::Response;
use crate::remote::query::github_list_members;
use crate::remote::query::github_list_pipelines;
use crate::remote::{
    query, Member, MergeRequestListBodyArgs, MergeRequestState, Pipeline, PipelineBodyArgs, Project,
};
use crate::Result;
use std::sync::Arc;

pub mod merge_request;

#[derive(Clone)]
pub struct Github<R> {
    api_token: String,
    domain: String,
    path: String,
    rest_api_basepath: String,
    runner: Arc<R>,
}

impl<R> Github<R> {
    pub fn new(config: impl ConfigProperties, domain: &str, path: &str, runner: Arc<R>) -> Self {
        let api_token = config.api_token().to_string();
        let domain = domain.to_string();
        let rest_api_basepath = format!("https://api.{}", domain);

        Github {
            api_token,
            domain,
            path: path.to_string(),
            rest_api_basepath,
            runner,
        }
    }

    fn request_headers(&self) -> Headers {
        let mut headers = Headers::new();
        let auth_token_value = format!("bearer {}", self.api_token);
        headers.set("Authorization".to_string(), auth_token_value);
        headers.set(
            "Accept".to_string(),
            "application/vnd.github.v3+json".to_string(),
        );
        headers.set("User-Agent".to_string(), "gg".to_string());
        headers
    }

    fn url_list_merge_requests(&self, args: &MergeRequestListBodyArgs) -> String {
        match args.state {
            MergeRequestState::Opened => {
                format!(
                    "{}/repos/{}/pulls?state=open",
                    self.rest_api_basepath, self.path
                )
            }
            // Github has no distinction between closed and merged. A merged
            // pull request is considered closed.
            MergeRequestState::Closed | MergeRequestState::Merged => {
                format!(
                    "{}/repos/{}/pulls?state=closed",
                    self.rest_api_basepath, self.path
                )
            }
        }
    }
}

impl<R: HttpRunner<Response = Response>> RemoteProject for Github<R> {
    fn get_project_data(&self, id: Option<i64>) -> Result<CmdInfo> {
        if let Some(id) = id {
            return Err(GRError::OperationNotSupported(format!(
                "Getting project data by id is not supported in Github: {}",
                id
            ))
            .into());
        };
        let url = format!("{}/repos/{}", self.rest_api_basepath, self.path);
        let project = query::github_project_data::<_, ()>(
            &self.runner,
            &url,
            None,
            self.request_headers(),
            GET,
            ApiOperation::Project,
        )?;
        Ok(CmdInfo::Project(project))
    }

    fn get_project_members(&self) -> Result<CmdInfo> {
        let url = &format!(
            "{}/repos/{}/contributors",
            self.rest_api_basepath, self.path
        );
        let members = github_list_members(
            &self.runner,
            url,
            None,
            self.request_headers(),
            None,
            ApiOperation::Project,
        )?;
        Ok(CmdInfo::Members(members))
    }

    fn get_url(&self, option: BrowseOptions) -> String {
        let base_url = format!("https://{}/{}", self.domain, self.path);
        match option {
            BrowseOptions::Repo => base_url,
            BrowseOptions::MergeRequests => format!("{}/pulls", base_url),
            BrowseOptions::MergeRequestId(id) => format!("{}/pull/{}", base_url, id),
            BrowseOptions::Pipelines => format!("{}/actions", base_url),
        }
    }
}

pub struct GithubProjectFields {
    id: i64,
    default_branch: String,
    html_url: String,
}

impl From<&serde_json::Value> for GithubProjectFields {
    fn from(project_data: &serde_json::Value) -> Self {
        GithubProjectFields {
            id: project_data["id"].as_i64().unwrap(),
            default_branch: project_data["default_branch"]
                .to_string()
                .trim_matches('"')
                .to_string(),
            html_url: project_data["html_url"]
                .to_string()
                .trim_matches('"')
                .to_string(),
        }
    }
}

impl From<GithubProjectFields> for Project {
    fn from(fields: GithubProjectFields) -> Self {
        Project::new(fields.id, &fields.default_branch).with_html_url(&fields.html_url)
    }
}

pub struct GithubMemberFields {
    id: i64,
    login: String,
    name: String,
}

impl From<&serde_json::Value> for GithubMemberFields {
    fn from(member_data: &serde_json::Value) -> Self {
        GithubMemberFields {
            id: member_data["id"].as_i64().unwrap(),
            login: member_data["login"].as_str().unwrap().to_string(),
            name: "".to_string(),
        }
    }
}

impl From<GithubMemberFields> for Member {
    fn from(fields: GithubMemberFields) -> Self {
        Member::builder()
            .id(fields.id)
            .username(fields.login)
            .name(fields.name)
            .build()
            .unwrap()
    }
}

impl<R: HttpRunner<Response = Response>> Cicd for Github<R> {
    fn list(&self, args: PipelineBodyArgs) -> Result<Vec<Pipeline>> {
        // Doc:
        // https://docs.github.com/en/rest/actions/workflow-runs?apiVersion=2022-11-28#list-workflow-runs-for-a-repository
        let url = format!(
            "{}/repos/{}/actions/runs",
            self.rest_api_basepath, self.path
        );
        github_list_pipelines(
            &self.runner,
            &url,
            args.from_to_page,
            self.request_headers(),
            Some("workflow_runs"),
            ApiOperation::Pipeline,
        )
    }

    fn get_pipeline(&self, _id: i64) -> Result<Pipeline> {
        todo!()
    }

    fn num_pages(&self) -> Result<Option<u32>> {
        let url = format!(
            "{}/repos/{}/actions/runs?page=1",
            self.rest_api_basepath, self.path
        );
        let headers = self.request_headers();
        query::num_pages(&self.runner, &url, headers, ApiOperation::Pipeline)
    }
}

pub struct GithubPipelineFields {
    status: String,
    web_url: String,
    branch: String,
    sha: String,
    created_at: String,
}

impl From<&serde_json::Value> for GithubPipelineFields {
    fn from(pipeline_data: &serde_json::Value) -> Self {
        GithubPipelineFields {
            // Github has `conclusion` as the final
            // state of the pipeline. It also has a
            // `status` field to represent the current
            // state of the pipeline. Our domain
            // `Pipeline` struct `status` refers to the
            // final state, i.e conclusion.
            status: pipeline_data["conclusion"]
                .as_str()
                // conclusion is not present when a
                // pipeline is running, gather its status.
                .unwrap_or_else(|| {
                    pipeline_data["status"]
                        .as_str()
                        // set is as unknown if
                        // neither conclusion nor
                        // status are present.
                        .unwrap_or("unknown")
                })
                .to_string(),
            web_url: pipeline_data["html_url"].as_str().unwrap().to_string(),
            branch: pipeline_data["head_branch"].as_str().unwrap().to_string(),
            sha: pipeline_data["head_sha"].as_str().unwrap().to_string(),
            created_at: pipeline_data["created_at"].as_str().unwrap().to_string(),
        }
    }
}

impl From<GithubPipelineFields> for Pipeline {
    fn from(fields: GithubPipelineFields) -> Self {
        Pipeline::builder()
            .status(fields.status)
            .web_url(fields.web_url)
            .branch(fields.branch)
            .sha(fields.sha)
            .created_at(fields.created_at)
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use crate::{
        error,
        remote::ListBodyArgs,
        test::utils::{config, get_contract, ContractType, MockRunner},
    };

    use super::*;

    #[test]
    fn test_get_project_data_no_id() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "project.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github = Github::new(config, &domain, &path, client.clone());
        github.get_project_data(None).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_project_data_with_id_not_supported() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let client = Arc::new(MockRunner::new(vec![]));
        let github = Github::new(config, &domain, &path, client.clone());
        assert!(github.get_project_data(Some(1)).is_err());
    }

    #[test]
    fn test_list_actions() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "list_pipelines.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let runs = github.list(args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/actions/runs",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
        assert_eq!(1, runs.len());
    }

    #[test]
    fn test_list_actions_error_status_code() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder().status(401).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        assert!(github.list(args).is_err());
    }

    #[test]
    fn test_list_actions_unexpected_ok_status_code() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder().status(302).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        match github.list(args) {
            Ok(_) => panic!("Expected error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::RemoteServerError(_)) => (),
                _ => panic!("Expected error::GRError::RemoteServerError"),
            },
        }
    }

    #[test]
    fn test_list_actions_empty_workflow_runs() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(r#"{"workflow_runs":[]}"#.to_string())
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        assert_eq!(0, github.list(args).unwrap().len());
    }

    #[test]
    fn test_workflow_runs_not_an_array_is_error() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(r#"{"workflow_runs":{}}"#.to_string())
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        match github.list(args) {
            Ok(_) => panic!("Expected error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::RemoteUnexpectedResponseContract(_)) => (),
                _ => panic!("Expected error::GRError::RemoteUnexpectedResponseContract"),
            },
        }
    }

    #[test]
    fn test_num_pages_for_list_actions() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let link_header = r#"<https://api.github.com/repos/jordilin/githapi/actions/runs?page=1>; rel="next", <https://api.github.com/repos/jordilin/githapi/actions/runs?page=1>; rel="last""#;
        let mut headers = Headers::new();
        headers.set("link".to_string(), link_header.to_string());
        let response = Response::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        assert_eq!(Some(1), github.num_pages().unwrap());
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/actions/runs?page=1",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    #[test]
    fn test_num_pages_error_retrieving_last_page() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder().status(200).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        assert!(github.num_pages().is_err());
    }

    #[test]
    fn test_list_actions_from_page_set_in_url() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "list_pipelines.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(Some(
                ListBodyArgs::builder()
                    .page(2)
                    .max_pages(3)
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();
        github.list(args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/actions/runs?page=2",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    #[test]
    fn test_list_actions_conclusion_field_not_available_use_status() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let contract_json = get_contract(ContractType::Github, "list_pipelines.json");
        let contract_json = contract_json
            .lines()
            .filter(|line| !line.contains("conclusion"))
            .collect::<Vec<&str>>()
            .join("\n");
        let response = Response::builder()
            .status(200)
            .body(contract_json)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let runs = github.list(args).unwrap();
        assert_eq!("completed", runs[0].status);
    }

    #[test]
    fn test_list_actions_neither_conclusion_nor_status_use_unknown() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let contract_json = get_contract(ContractType::Github, "list_pipelines.json");
        let contract_json = contract_json
            .lines()
            .filter(|line| !line.contains("conclusion") && !line.contains("status"))
            .collect::<Vec<&str>>()
            .join("\n");
        let response = Response::builder()
            .status(200)
            .body(contract_json)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let runs = github.list(args).unwrap();
        assert_eq!("unknown", runs[0].status);
    }
}
