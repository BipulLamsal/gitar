use crate::{
    http::Request,
    remote::{Member, MergeRequestResponse, Project},
    time::{now_epoch_seconds, Seconds},
    Result,
};
use regex::Regex;
use serde::Serialize;
use std::{collections::HashMap, ffi::OsStr};

pub trait Runner {
    type Response;
    fn run<T>(&self, cmd: T) -> Result<Self::Response>
    where
        T: IntoIterator,
        T::Item: AsRef<OsStr>;
}

pub trait HttpRunner {
    type Response;
    fn run<T: Serialize>(&self, cmd: &mut Request<T>) -> Result<Self::Response>;
    /// Return the number of API MAX PAGES allowed for the given Request.
    fn api_max_pages<T: Serialize>(&self, cmd: &Request<T>) -> u32;
}

#[derive(Debug)]
pub enum CmdInfo {
    StatusModified(bool),
    RemoteUrl { domain: String, path: String },
    Branch(String),
    LastCommitSummary(String),
    LastCommitMessage(String),
    Project(Project),
    Members(Vec<Member>),
    MergeRequest(MergeRequestResponse),
    MergeRequestsList(Vec<MergeRequestResponse>),
    OutgoingCommits(String),
    Ignore,
    Exit,
}

/// Adapts lower level I/O HTTP/Shell outputs to a common Response.
#[derive(Clone, Debug)]
pub struct Response {
    pub status: i32,
    pub body: String,
    /// Optional headers. Mostly used by HTTP downstream HTTP responses
    pub(crate) headers: Option<HashMap<String, String>>,
    link_header_processor: fn(&str) -> PageHeader,
    /// Default time in epoch seconds when the ratelimit is reset.
    time_to_ratelimit_reset: Seconds,
    remaining_requests: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResponseField {
    Body,
    Status,
    Headers,
}

impl Response {
    pub fn new() -> Self {
        Self {
            status: 0,
            body: String::new(),
            headers: None,
            link_header_processor: parse_link_headers,
            time_to_ratelimit_reset: now_epoch_seconds() + Seconds::new(60),
            // most limiting Github 5000/60 = 83.33 requests per minute. Round
            // up to 80.
            remaining_requests: 80,
        }
    }

    pub fn with_header_processor(mut self, processor: fn(&str) -> PageHeader) -> Self {
        self.link_header_processor = processor;
        self
    }

    pub fn with_status(mut self, status: i32) -> Self {
        self.status = status;
        self
    }

    pub fn with_body(mut self, output: String) -> Self {
        self.body = output;
        self
    }

    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = Some(headers);
        self
    }

    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers
            .as_ref()
            .and_then(|h| h.get(key))
            .map(|s| s.as_str())
    }

    pub fn get_page_headers(&self) -> Option<PageHeader> {
        if let Some(headers) = &self.headers {
            match headers.get(LINK_HEADER) {
                Some(link) => return Some((self.link_header_processor)(link)),
                None => return None,
            }
        }
        None
    }

    // Defaults:
    // https://docs.gitlab.com/ee/user/gitlab_com/index.html#gitlabcom-specific-rate-limits
    // https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28#primary-rate-limit-for-authenticated-users

    // Github 5000 requests per hour for authenticated users
    // Gitlab 2000 requests per minute for authenticated users
    // Most limiting Github 5000/60 = 83.33 requests per minute

    pub fn get_ratelimit_headers(&self) -> Option<RateLimitHeader> {
        let mut ratelimit_header = RateLimitHeader::default();

        // process remote headers and patch the defaults accordingly
        if let Some(headers) = &self.headers {
            if let Some(github_remaining) = headers.get(GITHUB_RATELIMIT_REMAINING) {
                ratelimit_header.remaining = github_remaining
                    .parse::<u32>()
                    .unwrap_or(self.remaining_requests);
                if let Some(github_reset) = headers.get(GITHUB_RATELIMIT_RESET) {
                    ratelimit_header.reset = Seconds::new(
                        github_reset
                            .parse::<u64>()
                            .unwrap_or(*self.time_to_ratelimit_reset),
                    );
                }
                return Some(ratelimit_header);
            }
            if let Some(gitlab_remaining) = headers.get(GITLAB_RATELIMIT_REMAINING) {
                ratelimit_header.remaining = gitlab_remaining
                    .parse::<u32>()
                    .unwrap_or(self.remaining_requests);
                if let Some(gitlab_reset) = headers.get(GITLAB_RATELIMIT_RESET) {
                    ratelimit_header.reset = Seconds::new(
                        gitlab_reset
                            .parse::<u64>()
                            .unwrap_or(*self.time_to_ratelimit_reset),
                    );
                }
                return Some(ratelimit_header);
            }
        }
        None
    }

    pub fn get_etag(&self) -> Option<&str> {
        self.header("etag")
    }

    pub fn status(&self) -> i32 {
        self.status
    }
}

impl Default for Response {
    fn default() -> Self {
        Self::new()
    }
}

const NEXT: &str = "next";
const LAST: &str = "last";
pub const LINK_HEADER: &str = "link";

pub fn parse_link_headers(link: &str) -> PageHeader {
    lazy_static! {
        static ref RE_URL: Regex = Regex::new(r#"<([^>]+)>;\s*rel="([^"]+)""#).unwrap();
        static ref RE_PAGE_NUMBER: Regex = Regex::new(r"[^(per_)]page=(\d+)").unwrap();
    }
    let mut page_header = PageHeader::new();
    for cap in RE_URL.captures_iter(link) {
        if cap.len() > 2 && &cap[2] == NEXT {
            let url = cap[1].to_string();
            for page_cap in RE_PAGE_NUMBER.captures_iter(&url) {
                if page_cap.len() == 2 {
                    let page_number = page_cap[1].to_string();
                    let page_number: u32 = page_number.parse().unwrap_or(0);
                    let page = Page::new(&url, page_number);
                    page_header.set_next_page(page);
                    continue;
                }
            }
        }
        // TODO pull code out - return a page and its type next or last.
        if cap.len() > 2 && &cap[2] == LAST {
            let url = cap[1].to_string();
            for page_cap in RE_PAGE_NUMBER.captures_iter(&url) {
                if page_cap.len() == 2 {
                    let page_number = page_cap[1].to_string();
                    let page_number: u32 = page_number.parse().unwrap_or(0);
                    let page = Page::new(&url, page_number);
                    page_header.set_last_page(page);
                }
            }
        }
    }
    page_header
}

#[derive(Default)]
pub struct PageHeader {
    pub next: Option<Page>,
    pub last: Option<Page>,
}

impl PageHeader {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_next_page(&mut self, page: Page) {
        self.next = Some(page);
    }

    pub fn set_last_page(&mut self, page: Page) {
        self.last = Some(page);
    }
}

#[derive(Debug, PartialEq)]
pub struct Page {
    pub url: String,
    pub number: u32,
}

impl Page {
    pub fn new(url: &str, number: u32) -> Self {
        Page {
            url: url.to_string(),
            number,
        }
    }
}

// https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28#exceeding-the-rate-limit

pub const GITHUB_RATELIMIT_REMAINING: &str = "x-ratelimit-remaining";
pub const GITHUB_RATELIMIT_RESET: &str = "x-ratelimit-reset";

// https://docs.gitlab.com/ee/administration/settings/user_and_ip_rate_limits.html

// Internal processing is all in lowercase
// Docs: RateLimit-Remaining
pub const GITLAB_RATELIMIT_REMAINING: &str = "ratelimit-remaining";
// Docs: RateLimit-Reset
pub const GITLAB_RATELIMIT_RESET: &str = "ratelimit-reset";

/// Unifies the different ratelimit headers available from the different remotes.
/// Github API ratelimit headers:
/// remaining: x-ratelimit-remaining
/// reset: x-ratelimit-reset
/// Gitlab API ratelimit headers:
/// remaining: RateLimit-Remaining
/// reset: RateLimit-Reset
#[derive(Clone, Debug, Default)]
pub struct RateLimitHeader {
    // The number of requests remaining in the current rate limit window.
    pub remaining: u32,
    // Unix time-formatted time when the request quota is reset.
    pub reset: Seconds,
}

impl RateLimitHeader {
    pub fn new(remaining: u32, reset: Seconds) -> Self {
        RateLimitHeader { remaining, reset }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_rate_limit_headers_github() {
        let body = "responsebody";
        let mut headers = HashMap::new();
        headers.insert("x-ratelimit-remaining".to_string(), "30".to_string());
        headers.insert("x-ratelimit-reset".to_string(), "1658602270".to_string());
        let response = Response::new()
            .with_body(body.to_string())
            .with_headers(headers);
        let ratelimit_headers = response.get_ratelimit_headers().unwrap();
        assert_eq!(30, ratelimit_headers.remaining.clone());
        assert_eq!(Seconds::new(1658602270), ratelimit_headers.reset);
    }

    #[test]
    fn test_get_rate_limit_headers_gitlab() {
        let body = "responsebody";
        let mut headers = HashMap::new();
        headers.insert("ratelimit-remaining".to_string(), "30".to_string());
        headers.insert("ratelimit-reset".to_string(), "1658602270".to_string());
        let response = Response::new()
            .with_body(body.to_string())
            .with_headers(headers);
        let ratelimit_headers = response.get_ratelimit_headers().unwrap();
        assert_eq!(30, ratelimit_headers.remaining);
        assert_eq!(Seconds::new(1658602270), ratelimit_headers.reset);
    }

    #[test]
    fn test_get_rate_limit_headers_camelcase_gitlab() {
        let body = "responsebody";
        let mut headers = HashMap::new();
        headers.insert("RateLimit-remaining".to_string(), "30".to_string());
        headers.insert("rateLimit-reset".to_string(), "1658602270".to_string());
        let response = Response::new()
            .with_body(body.to_string())
            .with_headers(headers);
        let ratelimit_headers = response.get_ratelimit_headers();
        assert!(ratelimit_headers.is_none());
    }
}
