use std::{time::Duration, sync::Arc, path::{Path, PathBuf}, cmp::Reverse};

use anyhow::Result;
use clap::Parser;
use futures::Future;
use pewter::{io::pgn::{parse_multi_pgn, Game}, engine::opening_db::OpeningDb, State};
use scraper::{Html, Selector};
use governor::{Quota, RateLimiter};
use rayon::prelude::*;

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

async fn get_all_games(cache_dir: &Path) -> Result<Vec<Game>> {
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
    
    let cache_dir = Box::new(cache_dir.to_path_buf());
    let cache_dir: &Path = Box::leak(cache_dir);
    
    let index_page = Html::parse_document(&index_page);
    let link_selector = Selector::parse("a").unwrap();
    let games = index_page.select(&link_selector)
        .filter_map(|l| l.value().attr("href"))
        .filter(|link| link.starts_with("events/"))
        .filter(|link| link.ends_with(".pgn"))
        .map(|link| format!("https://www.pgnmentor.com/{link}"))
        .map(|link| {
            let limiter = limiter.clone();
            tokio::spawn(async move {
                let pgn_data = get_pgn_data(&link, cache_dir, || limiter.until_ready()).await?;
                let mut games = parse_multi_pgn(&pgn_data)?;
                Result::<Vec<Game>>::Ok(games.drain(..).filter_map(|g| g.ok()).collect::<Vec<_>>())
            })
        })
        .collect::<Vec<_>>();

    let all_games = futures::future::join_all(games).await;
    println!("Loaded all PGN data, building opening DB");

    // Dismantle the various levels of Result that have accumulated so far, and flatten the games list
    let all_games = all_games.into_iter().collect::<Result<Vec<_>, _>>()?;
    let all_games = all_games.into_iter()
        .filter_map(|g| g.ok())
        .flat_map(|g| g)
        .collect::<Vec<_>>();
    
    Ok(all_games)
}

fn build_db_from_games(games: &[Game]) -> OpeningDb {
    println!("Building single DB from {} games", games.len());
    let mut db = games.par_iter()
        .fold(|| OpeningDb::new_empty(), |mut db, game| {
            db.add_game(game);
            db
        })
        .reduce(|| OpeningDb::new_empty(), |a, b| a.merge(b));

    println!("Finished building initial DB");
    
    println!("Filtering down DB");
    // Remove moves that didn't happen very often
    db.filter_moves(|r| r.total_count() > 20);
    db.prune(0);
    
    db
}

async fn save_db_to_disk(db: &OpeningDb, path: &Path) -> Result<()> {
    println!("Writing DB to {}", path.to_str().unwrap());
    let data = db.serialize()?;
    tokio::fs::write(path, &data).await?;
    println!("Done");

    Ok(())
}

async fn load_db_from_disk(path: &Path) -> Result<OpeningDb> {
    let data = tokio::fs::read(path).await?;
    Ok(OpeningDb::deserialize(&data)?)
}

/// Handles scraping pgnmentor.com, and building a pewter opening DB from those games
#[derive(Parser, Debug)]
#[clap(about, version, author, name="search_debugger")]
struct Args {
    /// Directory to cache downloaded PGN files when scraping
    #[clap(long)]
    pgn_cache: Option<PathBuf>,
    
    /// The path to read/write the opening DB from
    #[clap(long)]
    db_path: PathBuf,
    
    /// Just load the database from the given path, don't rebuild it from scratch
    #[clap(long)]
    no_build: bool,
    
    /// Dump the contents of the DB for the given FEN string
    #[clap(long)]
    debug_fen: Option<String>,
}

#[tokio::main]
async fn main()  -> Result<()> {
    let args = Args::parse();

    let db = if args.no_build {
        load_db_from_disk(&args.db_path).await?
    } else {
        let cache_dir = args.pgn_cache
            .expect("PGN cache directory required when building DB");
        let all_games = get_all_games(&cache_dir).await?;
        let db = build_db_from_games(&all_games);
        save_db_to_disk(&db, &args.db_path).await?;
        db
    };
    
    if let Some(debug_fen) = args.debug_fen {
        let state = pewter::io::fen::parse_fen(&debug_fen).unwrap();
        debug_print_db(&db, &state);
    }

    Ok(())
}

fn debug_print_db(db: &OpeningDb, state: &State) {
    let mut results = Vec::new();
    results.extend(db.query(state));
    results.sort_by_key(|r| Reverse(r.total_count()));
    
    println!("From this state ({:?} to play):", state.to_play);
    println!("{}", state.pretty_format());

    println!(" move   | wins   | losses | draws  | total");
    println!("--------+--------+--------+--------+-------");
    for r in results {
        let m = format!("{}", r.m);
        println!(" {:<6} | {:<6} | {:<6} | {:<6} | {}", m, r.wins, r.losses, r.draws, r.total_count())
    }
}