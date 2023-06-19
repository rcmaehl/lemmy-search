mod api;
mod config;
mod crawler;
mod database;

use std::env;
use actix_files as fs;
use actix_web::{
    App, 
    HttpServer
};
use api::search::SearchHandler;
use crawler::Runner;
use database::Database;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let ui_directory = match args.get(1) {
        Some(path) => path,
        None => "./ui"
    }.to_owned();

    let config = config::Config::load();

    let database = Database::new(config.postgres);
    let pool = database.build_database_pool()
        .await
        .unwrap();    

    let mut cralwer_runner = Runner::new(config.crawler, pool.clone());
    cralwer_runner.start();

    let factory = move || {
        let search_handler = SearchHandler::new();
        let mut app = App::new();
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