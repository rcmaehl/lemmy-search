pub mod models;

use regex::Regex;
use lazy_static::lazy_static;
use std::{
    collections::{
        HashMap, 
        HashSet
    }, 
    sync::Mutex, 
    time::Instant
};
use actix_web::{
    web::{
        Json, 
        Query,
        Data,
        get
    }, 
    Responder, 
    Result, 
    Route
};
use crate::{
    error::LogError,
    api::search::models::search::{
        SearchQuery,
        SearchResult
    }, 
    database::{
        dbo::{
            site::SiteDBO, 
            search::SearchDatabase
        }, 
        DatabasePool
    }, 
    crawler::crawler::Crawler, 
    config::Config
};

use self::models::search::Version;

lazy_static! {
    static ref INSTANCE_MATCH : Regex = Regex::new(r" instance:(?P<instance>(https://)?[\w\-\.]+)").unwrap();
    static ref COMMUNITY_MATCH : Regex = Regex::new(r" community:(?P<community>!\w+@[\w\-\.]+)").unwrap();
    static ref AUTHOR_MATCH : Regex = Regex::new(r" author:(?P<author>@\w+@[\w\-\.]+)").unwrap();

    static ref COMMUNITY_FORMAT : Regex = Regex::new(r"!(?P<name>\w+)@(?P<instance>[\w\-\.]+)").unwrap();
    static ref AUTHOR_FORMAT : Regex = Regex::new(r"@(?P<name>\w+)@(?P<instance>[\w\-\.]+)").unwrap();
}

pub struct SearchHandler {
    pub routes : HashMap<String, Route>
}

impl SearchHandler {

    const PAGE_LIMIT : usize = 50;

    pub fn new(config : &Config) -> Self {
        let mut routes = HashMap::<String, Route>::new();
        if config.development_mode {
            routes.insert("/heartbeat".to_string(), get().to(Self::heartbeat));
            routes.insert("/crawl".to_string(), get().to(Self::crawl));
        }
        routes.insert("/version".to_string(), get().to(Self::version));
        routes.insert("/search".to_string(), get().to(Self::search));
        routes.insert("/instances".to_string(), get().to(Self::get_instances));

        Self {
            routes
        }
    }

    pub async fn version<'a>(

    ) -> Result<impl Responder> {
        Ok(
            Json(
                Version {
                    version: env!("CARGO_PKG_VERSION").to_string()
                }
            ).customize()
            .insert_header(("cache-control", "public, max-age=86400"))
        )
    }

    /**
     * This method solely exists as just a way to confirm that the server is responding.
     * It should never do anything besides just respond with 'Ready'.
     */
    pub async fn heartbeat<'a>(
        
    ) -> Result<impl Responder> {
        Ok(
            "Ready"
                .customize()
                .insert_header(("cache-control", "no-store"))
        )
    }

    /**
     * Temporary endpoint to allow for more easily testing the crawler.
     */
    pub async fn crawl<'a>(
        pool : Data<Mutex<DatabasePool>>
    ) -> Result<impl Responder> {

        tokio::spawn(async move {

            let config = Config::load();

            let crawler = Crawler::new(
                config.crawler.seed_instance.clone(), 
                config.crawler, 
                pool.lock().unwrap().clone(), 
                false
            ).unwrap();

            let _ = crawler.crawl()
                .await
                .log_error("The manually triggered crawler encountered an error.", true);
        });

        Ok(
            "Started"
                .customize()
                .insert_header(("cache-control", "no-store"))
        )
    }

    /**
     * This is the actual search function that is called when the user enters a query.
     * 
     * This method will tokenize the query string and extract any filters provided by
     * the user before sending that information off to the Database to query.
     */
    pub async fn search<'a>(
        pool : Data<Mutex<DatabasePool>>,
        search_query: Query<SearchQuery>
    ) -> Result<impl Responder> {

        let start = Instant::now();

        let query = search_query.query.to_owned();
        let mut modified_query = query.clone();
        
        // Extract filters
        let instance = match INSTANCE_MATCH.captures(&query) {
            Some(caps) => {
                let cap = &caps["instance"].to_lowercase();
                modified_query = modified_query.replace(cap, "")
                    .replace("instance:", "");
                Some(if cap.starts_with("https://") {
                    cap.to_string()
                } else {
                    format!("https://{}/", cap)
                })
            },
            None => None
        };
        let community = match COMMUNITY_MATCH.captures(&query) {
            Some(caps) => {
                let cap = &caps["community"].to_lowercase();
                modified_query = modified_query.replace(cap, "")
                    .replace("community:", "");

                // Change the format from the user format of !name@instance
                // to match the actor_id format of a URL https://instance/c/name.
                match COMMUNITY_FORMAT.captures(&cap) {
                    Some(caps2) => {
                        let name = caps2["name"].to_lowercase();
                        let instance = caps2["instance"].to_lowercase();
                        Some(format!("https://{}/c/{}", instance, name))
                    },
                    None => None
                }
            },
            None => None
        };
        let author = match AUTHOR_MATCH.captures(&query) {
            Some(caps) => {
                let cap = &caps["author"].to_lowercase();
                modified_query = modified_query.replace(cap, "")
                    .replace("author:", "");
                
                // Change the format from the user format of @name@instance
                // to match the actor_id format of a URL https://instance/c/name.
                match AUTHOR_FORMAT.captures(&cap) {
                    Some(caps2) => {
                        let name = caps2["name"].to_lowercase();
                        let instance = caps2["instance"].to_lowercase();
                        Some(format!("https://{}/u/{}", instance, name))
                    },
                    None => None
                }
            },
            None => None
        };

        // normalize the query string to lowercase.
        modified_query = modified_query.to_lowercase()
            .trim()
            .to_string();

        // Log search query
        println!("Searching for '{}'", modified_query);
        match &instance {
            Some(value) => {
                println!("\tInstance: '{}'", value);
            },
            None => {}
        }
        match &community {
            Some(value) => {
                println!("\tCommunity: '{}'", value);
            },
            None => {}
        }
        match &author {
            Some(value) => {
                println!("\tAuthor: '{}'", value);
            },
            None => {}
        }

        // tokenize the search query, remove any non-alphanumeric characters from the string
        // and remove any words that are less than 3 characters long.
        let query_terms = modified_query.replace(|c : char| {
            !c.is_alphanumeric() && !c.is_whitespace()
        }, " ")
            .split_whitespace()
            .map(|word| {
                word.trim().to_string()
            }).filter(|word| {
                word.len() > 2
            }).collect::<HashSet<String>>();

        // The preferred instance is sent without the https://, re-add it back.
        let preferred_instance_actor_id = format!("https://{}/", search_query.preferred_instance);

        let page = search_query.page.unwrap_or(1).max(1);

        let search = SearchDatabase::new(pool.lock().unwrap().clone());
        let search_results = search.search(
            &query_terms, 
            &instance, 
            &community, 
            &author, 
            &preferred_instance_actor_id,
            page
        ).await
            .log_error("Error during search.", true)
            .map_err(|err| {
                actix_web::error::ErrorInternalServerError(err)
            })?;

        let len = search_results.1;
        let total_pages = (len as f32 / Self::PAGE_LIMIT as f32).ceil() as i32;

        // Capture the duration that the search took so we can report it back
        // to the user.
        let duration = start.elapsed();

        let results: SearchResult = SearchResult {
            original_query_terms : query_terms,
            posts : search_results.0,
            total_results : len,
            total_pages : total_pages,
            time_taken: duration
        };

        Ok(
            Json(results)
                .customize()
                .insert_header(("cache-control", "public, max-age=86400"))
        )
    }

    /**
     * Returns a list of all available instances that this search engine has seen.
     * 
     * These will be ultimately used as the 'preferred instance' when calling
     * the actual search method.
     */
    pub async fn get_instances<'a>(
        pool : Data<Mutex<DatabasePool>>
    ) -> Result<impl Responder> {
        let pool = pool.lock().unwrap();

        let sites = SiteDBO::new(pool.clone())
            .retrieve_all()
            .await.map_err(|err| {
                actix_web::error::ErrorInternalServerError(err)
            })?;

        Ok(
            Json(sites)
                .customize()
                .insert_header(("cache-control", "public, max-age=86400"))
        )
    }
}
