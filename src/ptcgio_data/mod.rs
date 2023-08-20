use anyhow::{Context, Result};
use git2::Repository;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default, Debug, Eq, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Card {
    pub id: String,
    pub name: String,
    pub supertype: String,
    pub subtypes: Option<Vec<String>>,
    pub level: Option<String>,
    pub hp: Option<String>,
    pub types: Option<Vec<String>>,
    pub evolves_from: Option<String>,
    pub evolves_to: Option<Vec<String>>,
    pub abilities: Option<Vec<BTreeMap<String, String>>>,
    pub rules: Option<Vec<String>>,
    pub attacks: Option<Vec<BTreeMap<String, serde_json::Value>>>,
    pub resistances: Option<Vec<BTreeMap<String, String>>>,
    pub weaknesses: Option<Vec<BTreeMap<String, String>>>,
    pub retreat_cost: Option<Vec<String>>,
    pub converted_retreat_cost: Option<usize>,
    pub number: String,
    pub artist: Option<String>,
    pub rarity: Option<String>,
    pub flavor_text: Option<String>,
    pub national_pokedex_numbers: Option<Vec<i32>>,
    pub legalities: BTreeMap<String, String>,
    pub images: BTreeMap<String, String>,
    pub ancient_trait: Option<BTreeMap<String, String>>,
    pub regulation_mark: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Set {
    #[serde(default)]
    pub cards: Vec<Card>,
    pub id: String,
    pub name: String,
    pub series: String,
    pub printed_total: i32,
    pub total: i32,
    pub legalities: HashMap<String, String>,
    pub ptcgo_code: Option<String>,
    pub release_date: String,
    pub updated_at: String,
    pub images: HashMap<String, String>,
}

impl Set {
    fn with_cards(&mut self, cards: Vec<Card>) {
        self.cards = cards;
    }
}

pub struct DataFetcher {
    url: String,
    path: PathBuf,
}

pub struct Data {
    path: PathBuf,
    pub sets: Vec<Set>,
}

const PTCG_DATA_URL: &str = "https://github.com/PokemonTCG/pokemon-tcg-data.git";

impl Default for DataFetcher {
    fn default() -> Self {
        DataFetcher::new(PTCG_DATA_URL, "pokemon-tcg-data")
    }
}

impl DataFetcher {
    pub fn new(data_url: &str, save_path: impl AsRef<Path>) -> Self {
        DataFetcher {
            url: data_url.to_string(),
            path: save_path.as_ref().to_path_buf(),
        }
    }

    pub fn fetch(self) -> Result<Data> {
        let res = self.fetch_();
        match res {
            Ok(data) => Ok(data),
            Err(err) => {
                fs::remove_dir_all(&self.path).expect("Failed to delete data directory");
                Err(err)
            }
        }
    }

    fn fetch_(&self) -> Result<Data> {
        Repository::clone(&self.url, &self.path).context("PTCG data repository failed to clone")?;

        let sets_file =
            File::open(self.path.join("sets/en.json")).context("Failed to open sets file")?;
        let reader = BufReader::new(sets_file);
        let sets: Vec<Set> = serde_json::from_reader::<BufReader<File>, Vec<Set>>(reader)
            .context("Failed to parse the sets file")?
            .into_iter()
            .map(|mut set| {
                let cards_file = File::open(
                    self.path
                        .join("cards/en")
                        .join(&set.id)
                        .with_extension("json"),
                )
                .context(format!("Failed to open {} cards file", &set.id))?;
                let r = BufReader::new(cards_file);
                let cards = serde_json::from_reader::<BufReader<File>, Vec<Card>>(r)
                    .context(format!("Failed to parse {} cards file", &set.id))?;

                set.with_cards(cards);
                Ok(set)
            })
            .collect::<Result<Vec<Set>>>()?;

        Ok(Data {
            path: self.path.clone(),
            sets,
        })
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).expect("Failed to delete data directory");
    }
}
