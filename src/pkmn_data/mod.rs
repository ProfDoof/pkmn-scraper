mod card;
mod extractors;
mod series;
mod set;

use crate::pkmn_data::series::{Series, SeriesFetcher};
use anyhow::{bail, Result};
use futures::stream::FuturesOrdered;
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use reqwest_middleware::ClientBuilder;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use scraper::{Html, Selector};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tokio_stream::StreamExt;

pub struct DataFetcher {
    url: String,
    store_path: PathBuf,
}

const SETS_URL: &str = "https://pkmncards.com/sets/";

impl Default for DataFetcher {
    fn default() -> Self {
        DataFetcher::new(SETS_URL, &PathBuf::from_str("pkmn_data").unwrap())
    }
}

impl DataFetcher {
    pub fn new(data_start_url: &str, store_path: &Path) -> Self {
        DataFetcher {
            url: data_start_url.to_string(),
            store_path: store_path.to_path_buf(),
        }
    }

    pub async fn fetch(self) -> Result<Data> {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(2);
        let client = ClientBuilder::new(
            reqwest::Client::builder()
                .pool_max_idle_per_host(0)
                .build()?,
        )
        .with(Cache(HttpCache {
            mode: CacheMode::Default,
            manager: CACacheManager::default(),
            options: HttpCacheOptions::default(),
        }))
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();

        let result = client.get(&self.url).send().await?;

        let sets_html = if result.status().is_success() {
            result.text().await?
        } else {
            bail!(
                "Error when getting page of all sets from {}: {}",
                &self.url,
                result.status()
            );
        };
        let set_doc = Html::parse_document(&sets_html);

        let series_selector = Selector::parse("h2 > a").unwrap();

        let series_fetchers = set_doc
            .select(&series_selector)
            .map(|block| SeriesFetcher::new(block, &client))
            .collect::<Result<Vec<SeriesFetcher>>>()?;

        if !&self.store_path.exists() {
            tokio::fs::create_dir_all(&self.store_path).await?;
        }

        let mut all_series = Vec::new();

        for series_future in series_fetchers
            .iter()
            .map(|fetcher| fetcher.fetch(&self.store_path))
        {
            all_series.push(series_future.await?);
        }

        Ok(Data { all_series })
    }
}

#[derive(Debug)]
pub struct Data {
    pub all_series: Vec<Series>,
}
