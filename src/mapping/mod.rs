use crate::ptcgio_data::Card;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::iter;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};

#[derive(Serialize, Deserialize)]
pub struct DatasetMappings {
    pub ptcgio: MappingOperations,
    pub pkmn: MappingOperations,
}

impl DatasetMappings {
    pub async fn load(path: impl AsRef<Path>) -> Result<DatasetMappings> {
        let file = File::open(path).await?;
        let mut reader = BufReader::new(file);
        let mut buffer: Vec<u8> = Vec::new();
        reader.read_to_end(&mut buffer).await?;
        Ok(serde_json::from_slice(&buffer)?)
    }
}

#[derive(Serialize, Deserialize)]
pub struct MappingOperations {
    ignore: HashSet<String>,
    merge: HashMap<String, String>,
    map: HashMap<String, String>,
    extract: HashMap<String, ExtractOp>,
}

impl MappingOperations {
    pub async fn map(
        &self,
        mut current_sets: BTreeMap<String, Vec<Card>>,
    ) -> Result<BTreeMap<String, Vec<Card>>> {
        // Extract step
        for (extracted_name, operation) in &self.extract {
            if current_sets.contains_key(extracted_name) {
                bail!("This dataset already has the set {}", extracted_name);
            }
            let mut extracted_vec: Vec<Option<Card>> = Vec::with_capacity(operation.count);
            extracted_vec.extend(iter::repeat(None).take(operation.count));
            for card_group in &operation.cards {
                let origin_set = current_sets.get(&card_group.from).ok_or(anyhow!(
                    "Attempted to extract a pokemon from a set not contained in this dataset: {}",
                    card_group.from
                ))?;

                for (target, source) in &card_group.numbers {
                    extracted_vec[*target] = Some(origin_set[*source].clone());
                }
            }

            current_sets.insert(
                extracted_name.to_string(),
                extracted_vec
                    .into_iter()
                    .enumerate()
                    .map(|(idx, opt)| opt.ok_or(anyhow!("Card at index {} was not filled", idx)))
                    .collect::<Result<Vec<Card>>>()?,
            );
        }

        // Map step
        for (source_name, target_name) in &self.map {
            let moving = current_sets.remove(source_name).ok_or(anyhow!(
                "Attempted to map {} to {} but {} did not exist in the dataset",
                source_name,
                target_name,
                target_name
            ))?;
            current_sets.insert(target_name.to_string(), moving);
        }

        // Merge Step
        for (source_name, target_name) in &self.merge {
            let moving = current_sets.remove(source_name).ok_or(anyhow!(
                "The source set {} for merging did not exist",
                source_name
            ))?;
            current_sets
                .get_mut(target_name)
                .ok_or(anyhow!("The target set {} did not exist", target_name))?
                .extend(moving);
        }

        // Ignore Step
        for set_name in &self.ignore {
            current_sets.remove(set_name).ok_or(anyhow!(
                "The set {} you wanted to ignore does not exist",
                set_name
            ))?;
        }

        Ok(current_sets)
    }
}

#[derive(Serialize, Deserialize)]
struct ExtractOp {
    count: usize,
    cards: Vec<ExtractGroup>,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
struct ExtractGroup {
    from: String,
    #[serde_as(as = "HashMap<DisplayFromStr, DisplayFromStr>")]
    numbers: HashMap<usize, usize>,
}
