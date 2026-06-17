use anyhow::Result;
use clap::{Parser, Subcommand};
use local_knowledge_search::app::AppState;
use local_knowledge_search::config::{default_config_path, AppConfig};
use local_knowledge_search::document::{read_preview, DocumentLoader};
use local_knowledge_search::indexer::KnowledgeIndex;
use local_knowledge_search::storage::LocalStore;
use local_knowledge_search::tui;
use local_knowledge_search::utils::bytes_to_human;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "local-knowledge-search")]
#[command(
    about = "Local knowledge base search system with CLI, TUI, high-performance indexing and local data management"
)]
struct Cli {
    #[arg(short, long, default_value = "./knowledge_config.json")]
    config: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create default configuration file
    Init,

    /// Rebuild full index from document directory
    Index {
        #[arg(short, long)]
        docs: Option<PathBuf>,
    },

    /// Incrementally update changed documents
    Update {
        #[arg(short, long)]
        docs: Option<PathBuf>,
    },

    /// Search in command line mode
    Search {
        query: String,
        #[arg(short, long)]
        limit: Option<usize>,
        #[arg(long)]
        json: bool,
    },

    /// Preview a local document
    Preview {
        path: PathBuf,
        #[arg(short, long, default_value_t = 2000)]
        chars: usize,
    },

    /// Show index statistics
    Stats,

    /// Show search history
    History,

    /// Clear search history
    ClearHistory,

    /// Show bookmarks
    Bookmarks,

    /// Add bookmark manually
    Bookmark { title: String, path: PathBuf },

    /// Open terminal UI
    Tui,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if matches!(cli.command, Commands::Init) {
        let cfg = AppConfig::default();
        cfg.save(&cli.config)?;
        println!("Created config: {}", cli.config.display());
        return Ok(());
    }

    let mut config = AppConfig::load_or_create(&cli.config)
        .or_else(|_| AppConfig::load_or_create(&default_config_path()))?;
    let store = LocalStore::new(config.data_dir.clone())?;
    let index = KnowledgeIndex::open_or_create(&config.index_dir, &config)?;

    match cli.command {
        Commands::Init => unreachable!(),
        Commands::Index { docs } => {
            if let Some(docs) = docs {
                config.docs_dir = docs;
            }
            let loader = DocumentLoader::new(config.clone());
            let report = index.rebuild_from_dir(&config.docs_dir, &loader, &store)?;
            println!("Indexed {} documents", report.indexed_documents);
            println!("Total size: {}", bytes_to_human(report.total_bytes));
            println!("Source directory: {}", config.docs_dir.display());
        }
        Commands::Update { docs } => {
            if let Some(docs) = docs {
                config.docs_dir = docs;
            }
            let loader = DocumentLoader::new(config.clone());
            let report = index.incremental_update(&config.docs_dir, &loader, &store)?;
            println!("Discovered: {}", report.discovered);
            println!("Changed: {}", report.changed);
            println!("Unchanged: {}", report.unchanged);
            println!("Total size: {}", bytes_to_human(report.total_bytes));
        }
        Commands::Search { query, limit, json } => {
            let limit = limit.unwrap_or(config.default_limit);
            let (total, results) = index.search_with_count(&query, limit)?;
            store.push_history(query.clone(), total)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                println!(
                    "Found {} matching documents, showing up to {} results.\n",
                    total,
                    results.len()
                );
                let mut shown = 0;
                let mut seen_paths = std::collections::HashSet::new();

                for r in results.iter() {
                    let path_key = r.path.to_string_lossy().to_string();

                    if !seen_paths.insert(path_key) {
                        continue;
                    }

                    shown += 1;

                    println!("{}. {}", shown, r.title);
                    println!("   score: {:.2}", r.score);
                    println!("   matched words: {}", r.match_count);
                    println!(
                        "   type: .{} | size: {}",
                        r.extension,
                        bytes_to_human(r.bytes)
                    );
                    println!("   path: {}", r.path.display());
                    println!("   snippet: {}", r.snippet.replace("**", ""));
                    println!();
                }
            }
        }
        Commands::Preview { path, chars } => {
            println!("{}", read_preview(&path, chars)?);
        }
        Commands::Stats => {
            let stats = index.stats_from_manifest(&store)?;
            println!("Documents: {}", stats.documents);
            println!("Total size: {}", bytes_to_human(stats.total_bytes));
            for ext in stats.extensions {
                println!(
                    ".{}: {} files, {}",
                    ext.extension,
                    ext.count,
                    bytes_to_human(ext.bytes)
                );
            }
        }
        Commands::History => {
            let history = store.load_history()?;
            for h in history.iter().rev().take(50) {
                println!(
                    "{} | {} matches | {}",
                    h.query, h.result_count, h.searched_at
                );
            }
        }
        Commands::ClearHistory => {
            store.clear_history()?;
            println!("History cleared.");
        }
        Commands::Bookmarks => {
            let bookmarks = store.load_bookmarks()?;
            if bookmarks.is_empty() {
                println!("No bookmarks.");
            } else {
                for b in bookmarks {
                    println!("{} | {} | {}", b.title, b.path.display(), b.added_at);
                }
            }
        }
        Commands::Bookmark { title, path } => {
            let added = store.add_bookmark(title, path)?;
            println!(
                "{}",
                if added {
                    "Bookmark added."
                } else {
                    "Bookmark already exists."
                }
            );
        }
        Commands::Tui => {
            let app = AppState::new(index, store, config)?;
            tui::run(app)?;
        }
    }
    Ok(())
}
