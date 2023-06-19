pub mod dbo;

use std::thread;
use crate::{config::Postgres, database::dbo::{comment::CommentDBO, DBO, site::SiteDBO}};
use postgres::{
    NoTls, 
    Config,
    Error
};
use r2d2_postgres::{
    PostgresConnectionManager, 
    r2d2::Pool
};

pub type DatabasePool = Pool<PostgresConnectionManager<NoTls>>;

#[derive(Clone)]
pub struct Database {
    pub pool : DatabasePool
}

impl Database {

    pub async fn create(
        config : &Postgres
    ) -> Result<Self, r2d2_postgres::r2d2::Error> {
        Self::create_database_pool(config)
            .await
            .map(|pool| {
                Database {
                    pool
                }
            })
    }

    async fn create_database_pool(
        config : &Postgres
    ) -> Result<DatabasePool, r2d2_postgres::r2d2::Error> {
        let db_config = Config::new()
            .user(&config.user)
            .password(&config.password)
            .host(&config.hostname)
            .port(config.port)
            .dbname(&config.database)
            .to_owned();

        let manager = PostgresConnectionManager::new(
            db_config, NoTls            
        );
        Pool::new(manager)
    }

    pub async fn init_database(
        &self,
    ) -> Result<(), Error> {
        println!("Creating database...");

        println!("\tCreating COMMENTS table...");
        CommentDBO::new(self.pool.clone())
            .create_table_if_not_exists()
            .await;

        println!("\tCreating SITES table...");
        SiteDBO::new(self.pool.clone())
            .create_table_if_not_exists()
            .await;

        let pool = self.pool.clone();
        let _ = match thread::spawn(move || {
            let mut client = pool.get().unwrap();

            println!("\tCreating WORDS table...");
            client.batch_execute("
                CREATE TABLE IF NOT EXISTS words (
                    id              UUID PRIMARY KEY,
                    word            VARCHAR NOT NULL
                )
            ").unwrap();

            println!("\tCreating WORDS_XREF_POSTS table...");
            client.batch_execute("
                CREATE TABLE IF NOT EXISTS words_xref_posts (
                    id              UUID PRIMARY KEY,
                    word_id         UUID NOT NULL,
                    post_id         UUID NOT NULL
                )
            ").unwrap();

            println!("\tCreating POSTS table...");
            client.batch_execute("
                CREATE TABLE IF NOT EXISTS posts (
                    id              UUID PRIMARY KEY,
                    title           VARCHAR NOT NULL,
                    body            VARCHAR NULL,
                    upvotes         INTEGER,
                    last_update     DATE
                )
            ").unwrap();

            
            client.batch_execute("
                CREATE TABLE IF NOT EXISTS comments (
                    id              UUID PRIMARY KEY,
                    post_id         UUID NOT NULL,
                    body            VARCHAR NULL,
                    upvotes         INTEGER,
                    last_update     DATE
                )
            ").unwrap();

            println!("\tCreating SITES table...");
            client.batch_execute("
                CREATE TABLE IF NOT EXISTS sites (
                    id              UUID PRIMARY KEY,
                    name            VARCHAR NULL,
                    actor_id        VARCHAR NOT NULL,
                    last_update     DATE
                )
            ").unwrap();
        }).join() {
            Ok(_) => {
                println!("Database creation complete...");
            },
            Err(_) => {
                println!("Database creation failed!");
            },
        };

        Ok(())
    }
}
