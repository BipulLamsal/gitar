use crate::api_traits::Cicd;
use crate::cli::PipelineOptions;
use crate::config::Config;
use crate::{remote, Result};
use std::io::Write;
use std::sync::Arc;

#[derive(Builder)]
pub struct ListPipelineCliArgs {
    pub from_page: Option<i64>,
    pub to_page: Option<i64>,
    pub refresh_cache: bool,
}

impl ListPipelineCliArgs {
    pub fn builder() -> ListPipelineCliArgsBuilder {
        ListPipelineCliArgsBuilder::default()
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
            list_pipelines(remote, std::io::stdout())
        }
    }
}

fn list_pipelines<W: Write>(remote: Arc<dyn Cicd>, mut writer: W) -> Result<()> {
    let pipelines = remote.list_pipelines()?;
    if pipelines.is_empty() {
        writer.write_all(b"No pipelines found.\n")?;
        return Ok(());
    }
    writer.write_all(b"URL | Branch | SHA | Created at | Status\n")?;
    for pipeline in pipelines {
        writer.write_all(format!("{}\n", pipeline).as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::error;
    use crate::remote::Pipeline;

    use super::*;

    #[derive(Clone, Builder)]
    struct PipelineList {
        #[builder(default = "vec![]")]
        pipelines: Vec<Pipeline>,
        #[builder(default = "false")]
        error: bool,
    }

    impl PipelineList {
        pub fn builder() -> PipelineListBuilder {
            PipelineListBuilder::default()
        }
    }

    impl Cicd for PipelineList {
        fn list_pipelines(&self) -> Result<Vec<Pipeline>> {
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
    }

    #[test]
    fn test_list_pipelines() {
        let pp_remote = PipelineList::builder()
            .pipelines(vec![
                Pipeline::builder()
                    .status("success".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/123".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .build()
                    .unwrap(),
                Pipeline::builder()
                    .status("failed".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/456".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap();
        let mut buf = Vec::new();
        list_pipelines(Arc::new(pp_remote), &mut buf).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "URL | Branch | SHA | Created at | Status\n\
             https://gitlab.com/owner/repo/-/pipelines/123 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | success\n\
             https://gitlab.com/owner/repo/-/pipelines/456 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | failed\n")
    }

    #[test]
    fn test_list_pipelines_empty() {
        let pp_remote = PipelineListBuilder::default().build().unwrap();
        let mut buf = Vec::new();
        list_pipelines(Arc::new(pp_remote), &mut buf).unwrap();
        assert_eq!("No pipelines found.\n", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_list_pipelines_error() {
        let pp_remote = PipelineListBuilder::default().error(true).build().unwrap();
        let mut buf = Vec::new();
        assert!(list_pipelines(Arc::new(pp_remote), &mut buf).is_err());
    }
}
