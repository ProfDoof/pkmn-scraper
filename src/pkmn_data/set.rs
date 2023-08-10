use super::card::CardFetcher;
use crate::ptcgio_data::Card;
use anyhow::{bail, Result};
use futures::stream::FuturesOrdered;
use heck::ToSnekCase;
use itertools::Itertools;
use regex::Regex;
use std::fmt::{Display, Formatter};
use std::iter;
use std::path::Path;
use tokio::fs::File;

use reqwest_middleware::ClientWithMiddleware;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio_stream::StreamExt;

pub(super) struct SetFetcher {
    url: String,
    pub set_name: String,
    set_code: Option<String>,
    client: ClientWithMiddleware,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Set {
    pub name: String,
    pub cards: Vec<Card>,
}

impl Display for Set {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut output = String::new();
        for out_str in self
            .cards
            .iter()
            .map(|card| card.name.as_str())
            .intersperse(", ")
        {
            output.push_str(out_str);
        }

        write!(f, "{} Cards: ({})", &self.name, output)
    }
}

impl Set {
    pub fn new(name: &str, cards: Vec<Card>) -> Self {
        Self {
            name: name.to_string(),
            cards,
        }
    }
}

impl SetFetcher {
    pub(super) fn new(set_ref: ElementRef, client: &ClientWithMiddleware) -> Result<Self> {
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
            set_code,
            client: client.clone(),
        })
    }

    pub(super) async fn fetch(&self, series: &Path) -> Result<Set> {
        let path = series.join(self.set_name.to_snek_case());
        if path.exists() {
            let file = File::open(path).await?;
            let mut reader = BufReader::new(file);
            let mut buffer: Vec<u8> = Vec::new();
            reader.read_to_end(&mut buffer).await?;
            Ok(serde_json::from_slice(&buffer)?)
        } else {
            let fetchers = self.get_card_fetchers().await?;
            let fetched = fetchers
                .iter()
                .map(|fetcher| fetcher.fetch())
                .collect::<FuturesOrdered<_>>();

            let set = Set::new(
                &self.set_name,
                fetched.collect::<Result<Vec<Card>>>().await?,
            );

            let file = File::create(path).await?;
            let mut writer = BufWriter::new(file);
            println!("Set: {}", &set);
            let output = serde_json::to_vec(&set)?;
            writer.write_all(&output).await?;
            writer.flush().await?;
            Ok(set)
        }
    }

    async fn get_card_fetchers(&self) -> Result<Vec<CardFetcher>> {
        let card_selector =
            Selector::parse("article.type-pkmn_card > div.entry-content > a.card-image-link, article.type-pkmn_card > div > div.card-text-area > header > div.card-title-meta > div > div.card-title-admin-links > h2 > a")
                .unwrap();
        let result = self.client.get(&self.url).send().await?;
        let set_page = if result.status().is_success() {
            result.text().await?
        } else {
            bail!(
                "Failed to get page of cards from set {} ({}) at {}: {}",
                self.set_name,
                self.set_code.as_ref().unwrap_or(&"UNK".to_string()),
                &self.url,
                result.status()
            )
        };
        let doc = Html::parse_document(&set_page);
        Ok(doc
            .select(&card_selector)
            .map(|card_ref| CardFetcher::new(card_ref, &self.client))
            .collect())
    }
}
