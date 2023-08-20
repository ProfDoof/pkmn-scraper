use super::set::SetFetcher;
use crate::pkmn_data::set::Set;
use anyhow::Result;
use heck::ToSnekCase;
use reqwest_middleware::ClientWithMiddleware;
use scraper::{ElementRef, Selector};
use selectors::Element;
use std::path::Path;

pub(super) struct SeriesFetcher {
    series: String,
    set_fetchers: Vec<SetFetcher>,
}

#[derive(Debug)]
pub struct Series {
    pub name: String,
    pub sets: Vec<Set>,
}

impl Series {
    fn new(name: &str, sets: Vec<Set>) -> Self {
        Self {
            name: name.to_string(),
            sets,
        }
    }
}

impl SeriesFetcher {
    pub(super) fn new(series_ref: ElementRef, client: &ClientWithMiddleware) -> Result<Self> {
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

    pub(super) async fn fetch(&self, base_path: &Path) -> Result<Series> {
        let path = base_path.join(self.series.to_snek_case());
        if !path.exists() {
            tokio::fs::create_dir(&path).await?;
        }

        let mut sets = Vec::new();

        for set_fetcher in self.set_fetchers.iter() {
            match set_fetcher.fetch(&path).await {
                Ok(set) => sets.push(set),
                Err(e) => eprintln!("Error when generating set {} \n########################################################{:?}\n########################################################\n", set_fetcher.set_name, e),
            };
        }
        Ok(Series::new(&self.series, sets))
    }
}
