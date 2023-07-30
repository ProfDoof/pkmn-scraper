use super::set::SetFetcher;
use crate::pkmn_data::set::Set;
use anyhow::Result;
use futures::stream::FuturesUnordered;
use reqwest::Client;
use scraper::{ElementRef, Selector};
use selectors::Element;
use tokio_stream::StreamExt;

pub(super) struct SeriesFetcher {
    series: String,
    set_fetchers: Vec<SetFetcher>,
}

pub(super) struct Series {
    pub _name: String,
    pub _sets: Vec<crate::pkmn_data::set::Set>,
}

impl Series {
    fn new(name: &str, sets: Vec<crate::pkmn_data::set::Set>) -> Self {
        Self {
            _name: name.to_string(),
            _sets: sets,
        }
    }
}

impl SeriesFetcher {
    pub(super) fn new(series_ref: ElementRef, client: &Client) -> Result<Self> {
        let raw_block_name = series_ref.inner_html();
        let series_name = html_escape::decode_html_entities(&raw_block_name).to_string();
        log::trace!("{}", series_name);

        let sets = series_ref
            .parent_element()
            .unwrap()
            .next_sibling_element()
            .unwrap();
        let set_selector = Selector::parse("li > a").unwrap();
        Ok(SeriesFetcher {
            series: series_name,
            set_fetchers: sets
                .select(&set_selector)
                .map(|set| SetFetcher::new(set, client))
                .collect::<Result<Vec<SetFetcher>>>()?,
        })
    }

    pub(super) async fn fetch(&self) -> Result<Series> {
        Ok(Series::new(
            &self.series,
            self.set_fetchers
                .iter()
                .take(1) // TODO: Remove Debug Limiters
                .map(|fetcher| fetcher.fetch())
                .collect::<FuturesUnordered<_>>()
                .collect::<Result<Vec<Set>>>()
                .await?,
        ))
    }
}
