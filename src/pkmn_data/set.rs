use super::card::CardFetcher;
use crate::ptcgio_data::Card;
use anyhow::Result;
use futures::stream::FuturesUnordered;
use regex::Regex;
use reqwest::Client;
use scraper::{ElementRef, Html, Selector};
use tokio_stream::StreamExt;

pub(super) struct SetFetcher {
    url: String,
    set_name: String,
    _set_code: Option<String>,
    client: Client,
}

pub struct Set {
    pub name: String,
    pub cards: Vec<crate::pkmn_data::Card>,
}

impl Set {
    pub fn new(name: &str, cards: Vec<crate::pkmn_data::Card>) -> Self {
        Self {
            name: name.to_string(),
            cards,
        }
    }
}

impl SetFetcher {
    pub(super) fn new(set_ref: ElementRef, client: &Client) -> Result<Self> {
        let re = Regex::new(r"(?<set_name>.*?)(\s\((?<set_code>.*)\))?$")?;
        let raw_set_name = set_ref.inner_html();
        let set_name_and_code = html_escape::decode_html_entities(&raw_set_name).to_string();
        log::trace!("set_name_and_code: {set_name_and_code}");
        let captures = re.captures(&set_name_and_code).unwrap();
        let set_name = captures["set_name"].to_string();
        let set_code = captures.name("set_code").map(|m| m.as_str().to_string());
        let url = set_ref.value().attr("href").unwrap().to_string();
        log::trace!("url: {url}, set_name: {set_name}, set_code: {set_code:?}");
        Ok(SetFetcher {
            url,
            set_name,
            _set_code: set_code,
            client: client.clone(),
        })
    }

    pub(super) async fn fetch(&self) -> Result<Set> {
        let fetchers = self.get_card_fetchers().await?;
        let fetched = fetchers
            .iter()
            .take(1) // TODO: Remove Debug Limiters
            .map(|fetcher| fetcher.fetch())
            .collect::<FuturesUnordered<_>>();

        Ok(Set::new(
            &self.set_name,
            fetched.collect::<Result<Vec<Card>>>().await?,
        ))
    }

    async fn get_card_fetchers(&self) -> Result<Vec<CardFetcher>> {
        let card_selector =
            Selector::parse("article.type-pkmn_card > div.entry-content > a.card-image-link")
                .unwrap();
        let set_page = self.client.get(&self.url).send().await?.text().await?;
        let doc = Html::parse_document(&set_page);
        Ok(doc
            .select(&card_selector)
            .map(|card_ref| CardFetcher::new(card_ref, &self.client))
            .collect())
    }
}
