use std::fmt::Debug;
use reqwest::Client;
use robotstxt::DefaultMatcher;
use crate::error::{
    Result,
    LemmySearchError
};
use serde::{
    Serialize, 
    de::DeserializeOwned
};
use super::models::{
    common::SortType,
    site::{
        SiteRequest,
        SiteResponse, 
        FederatedInstancesResponse, 
        FederatedInstancesRequest
    },
    post::{
        PostData, 
        PostListRequest, 
        PostListResponse, 
    }
};

pub struct Fetcher {
    instance : String,
    client : Client
}

impl Fetcher {

    pub const DEFAULT_LIMIT : i32 = 50;

    pub fn new(
        client : Client,
        instance : String
    ) -> Self {
        Self {
            client,
            instance
        }
    }

    fn get_url(
        &self,
        path : &str
    ) -> String {
        return format!("https://{}{}", self.instance, path);
    }

    pub async fn fetch_if_can_crawl(
        &self,
        user_agent : &str
    ) -> Result<bool> {

        let url = self.get_url("/robots.txt");

        println!("Connecting to {}...", url);
    
        let robots_txt = self.client
            .get(url)
            .send()
            .await?
            .text()
            .await?;

        Ok(DefaultMatcher::default().one_agent_allowed_by_robots(&robots_txt, user_agent, "/"))
    }

    pub async fn fetch_site_data(
        &self
    ) -> Result<SiteResponse> {
        let params = SiteRequest;
        let url = self.get_url("/api/v3/site");
        self.fetch_json::<SiteRequest, SiteResponse>(&url, params)
            .await
    }

    pub async fn fetch_instances(
        &self
    ) -> Result<FederatedInstancesResponse> {
        let params = FederatedInstancesRequest;
        let url = self.get_url("/api/v3/federated_instances");
        self.fetch_json(&url, params)
            .await
    }

    pub async fn fetch_posts(
        &self,
        page : i32
    ) -> Result<Vec<PostData>> {
        let params = PostListRequest {
            type_: Some(super::models::common::ListingType::All),
            sort: Some(SortType::Old),
            limit: Self::DEFAULT_LIMIT,
            page: page,
            ..Default::default()
        };

        let url = self.get_url("/api/v3/post/list");

        self.fetch_json(&url, params)
            .await
            .map(|view: PostListResponse| {
                view.posts
            })
    }

    async fn fetch_json<T, R>(
        &self,
        url : &str,
        params : T
    ) -> Result<R>
    where
        T : Serialize + Sized + Debug,
        R : Default + DeserializeOwned
    {
        println!("Connecting to {}...", url);
        println!("\twith params {:?}...", params);
    
        return match self.client
            .get(url)
            .query(&params)
            .send()
            .await {
                Ok(response) => {
                    response.json()
                        .await.map_err(|err| {
                            LemmySearchError::Network(err)
                        })
                }
                Err(err) => {
                    Err(LemmySearchError::Network(err))
                }
            }
    }
}
