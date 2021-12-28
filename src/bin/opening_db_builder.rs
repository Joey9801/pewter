use std::{time::Duration, sync::Arc, path::Path, collections::HashSet};

use anyhow::Result;
use futures::Future;
use pewter::{io::pgn::parse_multi_pgn, engine::opening_db::OpeningDb};
use scraper::{Html, Selector};
use governor::{Quota, RateLimiter};

async fn get_pgn_data<P: AsRef<Path>, F, G>(url: &str, cache_dir: P, governor: G) -> Result<String>
where
    F: Future<Output=()>,
    G: Fn() -> F
{
    let (_, filename) = url.rsplit_once("/").unwrap();

    let mut cache_filepath = cache_dir.as_ref().to_path_buf();
    cache_filepath.push(filename);

    if cache_filepath.exists() {
        return Ok(tokio::fs::read_to_string(cache_filepath).await?)
    }

    governor().await;
    println!("Making request to {url}");
    let pgn_data = reqwest::get(url)
        .await?
        .text()
        .await?;

    tokio::fs::create_dir_all(cache_filepath.parent().unwrap()).await?;
    tokio::fs::write(cache_filepath, &pgn_data).await?;

    Ok(pgn_data)
}

#[tokio::main]
async fn main() -> Result<()> {
    let quota = Quota::with_period(Duration::from_millis(750))
        .expect("Expected hard coded quota to be valid");
    let limiter = Arc::new(RateLimiter::direct(quota));

    limiter.until_ready().await;
    let index_url = "https://www.pgnmentor.com/files.html";
    println!("Making request to {index_url}");
    let index_page = reqwest::get(index_url)
        .await?
        .text()
        .await?;
    
    let index_page = Html::parse_document(&index_page);
    let link_selector = Selector::parse("a").unwrap();
    let links = index_page.select(&link_selector)
        .filter_map(|l| l.value().attr("href"))
        .filter(|link| link.starts_with("events/"))
        .filter(|link| link.ends_with(".pgn"))
        .map(|link| format!("https://www.pgnmentor.com/{link}"))
        .collect::<HashSet<String>>();

    let mut opening_db = OpeningDb::new_empty();
    let mut total_games = 0;
    for link in links.iter() {
        let pgn_data = get_pgn_data(link, "./pgn_cache", || limiter.until_ready()).await?;

        let games = parse_multi_pgn(&pgn_data)?;
        let mut this_games = 0;
        for game in games {
            match game {
                Ok(game) => {
                    opening_db.add_game(&game);
                    this_games += 1;
                }
                Err(e) => {
                    println!("Error parsing game from {link}: {e}");
                }
            }
        }
        total_games += this_games;
        println!("Got {} games from {} ({} total)", this_games, link, total_games);
    }

    Ok(())
}