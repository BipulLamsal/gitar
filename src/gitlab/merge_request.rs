use crate::api_traits::{ApiOperation, CommentMergeRequest, RemoteProject};
use crate::cli::browse::BrowseOptions;
use crate::cmds::merge_request::CommentMergeRequestBodyArgs;
use crate::error;
use crate::http::Method::GET;
use crate::http::{self, Body, Headers};
use crate::remote::{query, MergeRequestListBodyArgs};
use crate::Result;
use crate::{
    api_traits::MergeRequest,
    io::{HttpRunner, Response},
    remote::{MergeRequestBodyArgs, MergeRequestResponse},
};

use crate::json_loads;

use super::Gitlab;

impl<R: HttpRunner<Response = Response>> MergeRequest for Gitlab<R> {
    fn open(&self, args: MergeRequestBodyArgs) -> Result<MergeRequestResponse> {
        let mut body = Body::new();
        body.add("source_branch", args.source_branch);
        body.add("target_branch", args.target_branch);
        body.add("title", args.title);
        body.add("assignee_id", args.assignee_id);
        body.add("description", args.description);
        body.add("remove_source_branch", args.remove_source_branch);
        let url = format!("{}/merge_requests", self.rest_api_basepath());
        let response = query::gitlab_merge_request_response(
            &self.runner,
            &url,
            Some(body),
            self.headers(),
            http::Method::POST,
            ApiOperation::MergeRequest,
        )?;
        // if status code is 409, it means that the merge request already
        // exists. We already pushed the branch, just return the merge request
        // as if it was created.
        if response.status == 409 {
            // {\"message\":[\"Another open merge request already exists for
            // this source branch: !60\"]}"
            let merge_request_json: serde_json::Value = serde_json::from_str(&response.body)?;
            let merge_request_iid = merge_request_json["message"][0]
                .as_str()
                .unwrap()
                .split_whitespace()
                .last()
                .unwrap()
                .trim_matches('!');
            let merge_request_url = format!(
                "https://{}/{}/-/merge_requests/{}",
                self.domain, self.path, merge_request_iid
            );
            return Ok(MergeRequestResponse::builder()
                .id(merge_request_iid.parse().unwrap())
                .web_url(merge_request_url)
                .build()
                .unwrap());
        }
        if response.status != 201 {
            return Err(error::gen(format!(
                "Failed to open merge request: {}",
                response.body
            )));
        }
        let merge_request_json = json_loads(&response.body)?;

        Ok(MergeRequestResponse::builder()
            .id(merge_request_json["iid"].as_i64().unwrap())
            .web_url(merge_request_json["web_url"].as_str().unwrap().to_string())
            .build()
            .unwrap())
    }

    fn list(&self, args: MergeRequestListBodyArgs) -> Result<Vec<MergeRequestResponse>> {
        let url = self.list_merge_request_url(&args, false);
        query::gitlab_list_merge_requests(
            &self.runner,
            &url,
            args.list_args,
            self.headers(),
            None,
            ApiOperation::MergeRequest,
        )
    }

    fn merge(&self, id: i64) -> Result<MergeRequestResponse> {
        // PUT /projects/:id/merge_requests/:merge_request_iid/merge
        let url = format!("{}/merge_requests/{}/merge", self.rest_api_basepath(), id);
        query::gitlab_merge_request::<_, ()>(
            &self.runner,
            &url,
            None,
            self.headers(),
            http::Method::PUT,
            ApiOperation::MergeRequest,
        )
    }

    fn get(&self, id: i64) -> Result<MergeRequestResponse> {
        // GET /projects/:id/merge_requests/:merge_request_iid
        let url = format!("{}/merge_requests/{}", self.rest_api_basepath(), id);
        query::gitlab_merge_request::<_, ()>(
            &self.runner,
            &url,
            None,
            self.headers(),
            GET,
            ApiOperation::MergeRequest,
        )
    }

    fn close(&self, id: i64) -> Result<MergeRequestResponse> {
        let url = format!("{}/merge_requests/{}", self.rest_api_basepath(), id);
        let mut body = Body::new();
        body.add("state_event", "close");
        query::gitlab_merge_request::<_, &str>(
            &self.runner,
            &url,
            Some(body),
            self.headers(),
            http::Method::PUT,
            ApiOperation::MergeRequest,
        )
    }

    fn num_pages(&self, args: MergeRequestListBodyArgs) -> Result<Option<u32>> {
        let url = self.list_merge_request_url(&args, true);
        let mut headers = Headers::new();
        headers.set("PRIVATE-TOKEN", self.api_token());
        query::num_pages(&self.runner, &url, headers, ApiOperation::MergeRequest)
    }

    fn approve(&self, id: i64) -> Result<MergeRequestResponse> {
        let url = format!("{}/merge_requests/{}/approve", self.rest_api_basepath(), id);
        let result = query::gitlab_merge_request::<_, ()>(
            &self.runner,
            &url,
            None,
            self.headers(),
            http::Method::POST,
            ApiOperation::MergeRequest,
        );
        // responses in approvals for Gitlab do not contain the merge request
        // URL, patch it in the response.
        if let Ok(mut response) = result {
            response.web_url = self.get_url(BrowseOptions::MergeRequestId(id));
            return Ok(response);
        }
        result
    }
}

impl<R> Gitlab<R> {
    fn list_merge_request_url(&self, args: &MergeRequestListBodyArgs, num_pages: bool) -> String {
        let mut url = if let Some(assignee_id) = args.assignee_id {
            format!(
                "{}?state={}&assignee_id={}",
                self.merge_requests_url, args.state, assignee_id
            )
        } else {
            format!(
                "{}/merge_requests?state={}",
                self.rest_api_basepath(),
                args.state
            )
        };
        if num_pages {
            url.push_str("&page=1");
        }
        url
    }
}

impl<R: HttpRunner<Response = Response>> CommentMergeRequest for Gitlab<R> {
    fn create(&self, args: CommentMergeRequestBodyArgs) -> Result<()> {
        let url = format!(
            "{}/merge_requests/{}/notes",
            self.rest_api_basepath(),
            args.id
        );
        let mut body = Body::new();
        body.add("body", args.comment);
        query::create_merge_request_comment(
            &self.runner,
            &url,
            Some(body),
            self.headers(),
            http::Method::POST,
            ApiOperation::MergeRequest,
        )?;
        Ok(())
    }
}

pub struct GitlabMergeRequestFields {
    id: i64,
    web_url: String,
    source_branch: String,
    author: String,
    updated_at: String,
    created_at: String,
    title: String,
    description: String,
    merged_at: String,
    pipeline_id: Option<i64>,
    pipeline_url: Option<String>,
}

impl From<&serde_json::Value> for GitlabMergeRequestFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabMergeRequestFields {
            id: data["iid"].as_i64().unwrap_or_default(),
            web_url: data["web_url"].as_str().unwrap_or_default().to_string(),
            source_branch: data["source_branch"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            author: data["author"]["username"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            updated_at: data["updated_at"].as_str().unwrap_or_default().to_string(),
            created_at: data["created_at"].as_str().unwrap_or_default().to_string(),
            title: data["title"].as_str().unwrap_or_default().to_string(),
            description: data["description"].as_str().unwrap_or_default().to_string(),
            // If merge request is not merged, merged_at is an empty string.
            merged_at: data["merged_at"].as_str().unwrap_or_default().to_string(),
            // Documentation recommends gathering head_pipeline instead of
            // pipeline key.
            pipeline_id: data["head_pipeline"]["id"].as_i64(),
            pipeline_url: data["head_pipeline"]["web_url"]
                .as_str()
                .map(|s| s.to_string()),
        }
    }
}

impl From<GitlabMergeRequestFields> for MergeRequestResponse {
    fn from(fields: GitlabMergeRequestFields) -> Self {
        MergeRequestResponse::builder()
            .id(fields.id)
            .web_url(fields.web_url)
            .source_branch(fields.source_branch)
            .author(fields.author)
            .updated_at(fields.updated_at)
            .created_at(fields.created_at)
            .title(fields.title)
            .description(fields.description)
            .merged_at(fields.merged_at)
            .pipeline_id(fields.pipeline_id)
            .pipeline_url(fields.pipeline_url)
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {

    use std::sync::Arc;

    use crate::remote::{ListBodyArgs, MergeRequestState};
    use crate::test::utils::{config, get_contract, ContractType, MockRunner};

    use super::*;

    #[test]
    fn test_list_merge_request_with_from_page() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body("[]".to_string())
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(Some(
                ListBodyArgs::builder()
                    .page(2)
                    .max_pages(2)
                    .build()
                    .unwrap(),
            ))
            .assignee_id(None)
            .build()
            .unwrap();
        gitlab.list(args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=2",
            *client.url(),
        );
    }

    #[test]
    fn test_list_all_merge_requests_assigned_for_current_user() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body("[]".to_string())
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee_id(Some(1234))
            .build()
            .unwrap();
        gitlab.list(args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/merge_requests?state=opened&assignee_id=1234",
            *client.url(),
        );
    }

    #[test]
    fn test_open_merge_request() {
        let config = config();

        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();

        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi";
        let response = Response::builder()
            .status(201)
            .body(get_contract(ContractType::Gitlab, "merge_request.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client.clone());

        assert!(gitlab.open(mr_args).is_ok());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests",
            *client.url(),
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_open_merge_request_error() {
        let config = config();

        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(400).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client);
        assert!(gitlab.open(mr_args).is_err());
    }
    #[test]
    fn test_merge_request_already_exists_status_code_409_conflict() {
        let config = config();

        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();

        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(409)
            .body(get_contract(
                ContractType::Gitlab,
                "merge_request_conflict.json",
            ))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client);

        assert!(gitlab.open(mr_args).is_ok());
    }
    #[test]
    fn test_gitlab_merge_request_num_pages() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=1>; rel=\"next\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let response = Response::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee_id(None)
            .build()
            .unwrap();
        assert_eq!(Some(2), gitlab.num_pages(body_args).unwrap());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=1",
            *client.url(),
        );
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_current_auth_user() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let link_header = "<https://gitlab.com/api/v4/merge_requests?state=opened&assignee_id=1234&page=1>; rel=\"next\", <https://gitlab.com/api/v4/merge_requests?state=opened&assignee_id=1234&page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let response = Response::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee_id(Some(1234))
            .build()
            .unwrap();
        assert_eq!(Some(2), gitlab.num_pages(body_args).unwrap());
        assert_eq!(
            "https://gitlab.com/api/v4/merge_requests?state=opened&assignee_id=1234&page=1",
            *client.url(),
        );
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_no_link_header_error() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(200).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee_id(None)
            .build()
            .unwrap();
        assert_eq!(Some(1), gitlab.num_pages(body_args).unwrap());
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_response_error_is_error() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(400).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee_id(None)
            .build()
            .unwrap();
        assert!(gitlab.num_pages(body_args).is_err());
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_no_last_header_in_link() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=1>; rel=\"next\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let response = Response::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee_id(None)
            .build()
            .unwrap();
        assert_eq!(None, gitlab.num_pages(body_args).unwrap());
    }

    #[test]
    fn test_gitlab_create_merge_request_comment_ok() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(201).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn CommentMergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let comment_args = CommentMergeRequestBodyArgs::builder()
            .id(1456)
            .comment("LGTM, ship it".to_string())
            .build()
            .unwrap();
        gitlab.create(comment_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/1456/notes",
            *client.url()
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_gitlab_create_merge_request_comment_error() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(400).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn CommentMergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let comment_args = CommentMergeRequestBodyArgs::builder()
            .id(1456)
            .comment("LGTM, ship it".to_string())
            .build()
            .unwrap();
        assert!(gitlab.create(comment_args).is_err());
    }

    #[test]
    fn test_get_gitlab_merge_request_details() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "merge_request.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let merge_request_id = 123456;
        gitlab.get(merge_request_id).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/123456",
            *client.url()
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_merge_merge_request() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "merge_request.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let merge_request_id = 33;
        gitlab.merge(merge_request_id).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/33/merge",
            *client.url()
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_close_merge_request() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "merge_request.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let merge_request_id = 33;
        gitlab.close(merge_request_id).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/33",
            *client.url()
        );
        assert_eq!(http::Method::PUT, *client.http_method.borrow());
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_approve_merge_request_ok() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body(get_contract(
                ContractType::Gitlab,
                "approve_merge_request.json",
            ))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let merge_request_id = 33;
        let result = gitlab.approve(merge_request_id);
        match result {
            Ok(response) => {
                assert_eq!(
                    "https://gitlab.com/jordilin/gitlapi/-/merge_requests/33",
                    response.web_url
                );
            }
            Err(e) => {
                panic!(
                    "Expected Ok merge request approval but got: {:?} instead",
                    e
                );
            }
        }
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests/33/approve",
            *client.url()
        );
        assert_eq!(http::Method::POST, *client.http_method.borrow());
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }
}
