mod api;
mod config;
mod crawler;
mod database;
mod error;

use std::{
    env, 
    sync::Mutex, time::Duration
};
use actix_files as fs;
use actix_web::{
    App, 
    HttpServer,
    web::Data
};
use api::search::SearchHandler;
use crawler::Runner;
use database::Database;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let ui_directory = match args.get(1) {
        Some(path) => path,
        None => "./ui"
    }.to_owned();

    let config = config::Config::load();

    println!("Giving time for database to come online...");
    async_std::task::sleep(Duration::from_secs( 1 )).await;

    let database = match Database::create(&config.postgres).await {
        Ok(value) => value,
        Err(err) => {
            println!("Database pool creation failed...");
            if config.postgres.log {
                println!("{}", err);
            }
            panic!();
        }
    };

    let d = database.clone();

    let init_result = d.init_database().await;
    
    match init_result {
        Ok(_) => {}
        Err(err) => {
            println!("Database initialization failed...");
            if config.postgres.log {
                println!("{}", err);
            }
            panic!();
        }
    }

    let mut cralwer_runner = Runner::new(&config.crawler, database.clone());
    cralwer_runner.start();

    let pool = Data::new(Mutex::new(database.pool.clone()));

    let factory = move || {
        let search_handler = SearchHandler::new(&config);
        let mut app = App::new()
            .app_data(pool.clone());
        for (path, route) in search_handler.routes {
            app = app.route(path.as_str(), route);
        }
        app.service(
            fs::Files::new("/", &ui_directory)
                .index_file("index.html")
        )
    };

    let result = HttpServer::new(factory)
        .bind(("0.0.0.0", 8000))?
        .run()
        .await;

    cralwer_runner.stop();

    result
}
