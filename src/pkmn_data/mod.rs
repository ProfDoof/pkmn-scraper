mod card;
mod extractors;
mod series;
mod set;

use crate::pkmn_data::series::{Series, SeriesFetcher};
use crate::ptcgio_data::Card;
use anyhow::Result;
use futures::stream::FuturesUnordered;
use reqwest::Client;
use scraper::{Html, Selector};
use tokio_stream::StreamExt;

pub struct DataFetcher {
    url: String,
}

const SETS_URL: &str = "https://pkmncards.com/sets/";

impl Default for DataFetcher {
    fn default() -> Self {
        DataFetcher::new(SETS_URL)
    }
}

impl DataFetcher {
    pub fn new(data_start_url: &str) -> Self {
        DataFetcher {
            url: data_start_url.to_string(),
        }
    }

    pub async fn fetch(self) -> Result<Data> {
        let client = Client::new();

        let sets_html = client.get(&self.url).send().await?.text().await?;
        let set_doc = Html::parse_document(&sets_html);

        let series_selector = Selector::parse("h2 > a").unwrap();

        let series_fetchers = set_doc
            .select(&series_selector)
            .map(|block| SeriesFetcher::new(block, &client))
            .collect::<Result<Vec<SeriesFetcher>>>()?;

        Ok(Data {
            _all_series: series_fetchers
                .iter()
                .take(1) // TODO: Remove Debug Limiters
                .map(|fetcher| fetcher.fetch())
                .collect::<FuturesUnordered<_>>()
                .collect::<Result<Vec<Series>>>()
                .await?,
        })
    }
}

pub struct Data {
    _all_series: Vec<Series>,
}
