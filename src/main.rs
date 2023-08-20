mod diff;
mod mapping;
mod pkmn_data;
mod ptcgio_data;

use crate::diff::ValueIndex;
use crate::mapping::{DatasetMappings, MappingOperations};
use crate::ptcgio_data::Card;
use anyhow::{Context, Result};
use heck::ToSnekCase;
use itertools::Itertools;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::str::FromStr;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};

async fn filtering_example(data: ptcgio_data::Data) {
    let filtered_data = data
        .sets
        .iter()
        .flat_map(|set| {
            set.cards.iter().filter(|card| {
                card.resistances.as_ref().is_some_and(|resistances| {
                    resistances.iter().any(|map| {
                        map.get("value")
                            .is_some_and(|value| value != "-30" && value != "-20")
                    })
                }) || card.weaknesses.as_ref().is_some_and(|resistances| {
                    resistances
                        .iter()
                        .any(|map| map.get("value").is_some_and(|value| value != "×2"))
                })
            })
        })
        .collect::<Vec<&Card>>();

    println!("{:#?}\n{}", filtered_data, filtered_data.len());
}

fn extract_unique_sets(sets: HashMap<String, Vec<Vec<Card>>>) -> BTreeMap<String, Vec<Card>> {
    sets.into_iter()
        .map(|set| {
            let sets = set
                .1
                .into_iter()
                .unique_by(|cards| {
                    cards
                        .iter()
                        .sorted_by(|a, b| match Ord::cmp(&a.number, &b.number) {
                            Ordering::Less => Ordering::Less,
                            Ordering::Equal => Ord::cmp(&a.name, &b.name),
                            Ordering::Greater => Ordering::Greater,
                        })
                        .map(|card| card.name.clone())
                        .intersperse(", ".to_string())
                        .collect::<String>()
                })
                .collect_vec();
            if sets.len() > 1 || sets.is_empty() {
                (set.0, Vec::with_capacity(0))
            } else {
                (set.0, sets.into_iter().next().unwrap())
            }
        })
        .collect()
}

async fn process_dataset(
    dataset_iter: impl Iterator<Item = pkmn_data::Set>,
    mapping_operations: &MappingOperations,
) -> Result<BTreeMap<String, Vec<Card>>> {
    let data = dataset_iter
        .map(|set| {
            (
                set.name.clone(),
                set.cards
                    .iter()
                    .map(|card| {
                        let mut new_card = card.clone();
                        new_card.images.clear();
                        new_card.legalities.clear();
                        new_card.national_pokedex_numbers = None;
                        new_card
                    })
                    .collect::<Vec<Card>>(),
            )
        })
        .into_group_map();
    let data = extract_unique_sets(data);
    mapping_operations.map(data).await
}

async fn write_set_file(set: &HashSet<String>, file: File) -> Result<usize> {
    let ret = set.len();
    let mut writer = BufWriter::new(file);
    let output = serde_json::to_vec_pretty(&set)?;
    writer.write_all(&output).await?;
    writer.flush().await?;
    Ok(ret)
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let mapping_operations = DatasetMappings::load("mapping/set_operations.json").await?;
    const PERSONAL_DATA_URL: &str = "https://github.com/ProfDoof/pokemon-tcg-data.git";
    let mut ptcgio_data = ptcgio_data::DataFetcher::new(PERSONAL_DATA_URL, "ptcg-data").fetch()?;

    let ptcgio_data = process_dataset(
        ptcgio_data
            .sets
            .drain(..)
            .map(|set| pkmn_data::Set::new(set.name.as_str(), set.cards)),
        &mapping_operations.ptcgio,
    )
    .await?;

    let mut pkmn_data = pkmn_data::DataFetcher::default().fetch().await?;
    let pkmn_data = process_dataset(
        pkmn_data
            .all_series
            .drain(..)
            .flat_map(|series| series.sets),
        &mapping_operations.pkmn,
    )
    .await?;

    let diffs_dir = PathBuf::from_str("diffs")?;

    let pkmn_sets_set = pkmn_data.keys().cloned().collect::<HashSet<String>>();
    let ptcgio_sets_set = ptcgio_data.keys().cloned().collect::<HashSet<String>>();

    let intersect =
        extract_and_write_set_diffs(&diffs_dir, &pkmn_sets_set, &ptcgio_sets_set).await?;
    let combined_data = combine_data(ptcgio_data, pkmn_data, intersect)?;
    diff_data(diffs_dir, combined_data).await?;

    Ok(())
}

async fn extract_and_write_set_diffs(
    diffs_dir: &PathBuf,
    pkmn_sets_set: &HashSet<String>,
    ptcgio_sets_set: &HashSet<String>,
) -> Result<HashSet<String>> {
    tokio::fs::create_dir_all(&diffs_dir).await?;
    let sets_diffs = diffs_dir.join("sets");
    tokio::fs::create_dir_all(&sets_diffs).await?;

    let intersect: HashSet<_> = pkmn_sets_set
        .intersection(ptcgio_sets_set)
        .cloned()
        .collect();
    let pkmn_only: HashSet<_> = pkmn_sets_set.difference(ptcgio_sets_set).cloned().collect();
    let ptcgio_only: HashSet<_> = ptcgio_sets_set.difference(pkmn_sets_set).cloned().collect();

    let intersect_f = File::create(sets_diffs.join("intersect.json")).await?;
    let pkmn_only_f = File::create(sets_diffs.join("pkmn_only.json")).await?;
    let ptcgio_only_f = File::create(sets_diffs.join("ptcgio_only.json")).await?;

    println!(
        "Intersect size: {}",
        write_set_file(&intersect, intersect_f).await?
    );
    println!(
        "Pkmn Only size: {}",
        write_set_file(&pkmn_only, pkmn_only_f).await?
    );
    println!(
        "Ptcgio Only size: {}",
        write_set_file(&ptcgio_only, ptcgio_only_f).await?
    );
    Ok(intersect)
}

type PokemonNameBucket = BTreeMap<String, Vec<Card>>;
type CombinedSets = (PokemonNameBucket, PokemonNameBucket);

fn combine_data(
    mut ptcgio_data: BTreeMap<String, Vec<Card>>,
    mut pkmn_data: BTreeMap<String, Vec<Card>>,
    intersect: HashSet<String>,
) -> Result<HashMap<String, CombinedSets>> {
    intersect
        .iter()
        .map(|key| {
            Ok((
                key.clone(),
                ptcgio_data
                    .remove(key)
                    .context("Failed to get set from ptcgio_data")
                    .and_then(|val| {
                        Ok((
                            val.into_iter()
                                .into_group_map_by(|card| card.name.clone())
                                .into_iter()
                                .collect::<PokemonNameBucket>(),
                            pkmn_data
                                .remove(key)
                                .context("Failed to get set from pkmn_data")?
                                .into_iter()
                                .into_group_map_by(|card| card.name.clone())
                                .into_iter()
                                .collect::<PokemonNameBucket>(),
                        ))
                    })?,
            ))
        })
        .collect::<Result<HashMap<String, CombinedSets>>>()
}

async fn diff_data(diffs_dir: PathBuf, combined_data: HashMap<String, CombinedSets>) -> Result<()> {
    for (set_name, (ptcgio_data, pkmn_data)) in combined_data.into_iter() {
        let diff_dir = diffs_dir.join(set_name.to_snek_case());

        let calc_diff_log = diff_dir.join("changelog").with_extension("text");
        let data_log = diff_dir.join("data").with_extension("json");

        tokio::fs::create_dir_all(diff_dir).await?;

        let diff_log = File::create(calc_diff_log).await?;
        let data_log = File::create(data_log).await?;

        let merged_keys = ptcgio_data
            .keys()
            .chain(pkmn_data.keys())
            .sorted()
            .dedup()
            .collect_vec();

        let merged_data = merged_keys
            .iter()
            .map(|key| {
                let ptcgio: Value = serde_json::from_str(&serde_json::to_string_pretty(
                    &ptcgio_data.get(key.as_str()),
                )?)?;
                let pkmn: Value = serde_json::from_str(&serde_json::to_string_pretty(
                    &pkmn_data.get(key.as_str()),
                )?)?;

                Ok::<HashMap<&str, Value>, anyhow::Error>(HashMap::from([
                    ("ptcgio", sort_value(ptcgio)),
                    ("pkmn", sort_value(pkmn)),
                ]))
            })
            .collect::<Result<Vec<HashMap<&str, Value>>>>()?;
        let merged_data_str = serde_json::to_string_pretty(&merged_data)?;

        let mut writer = BufWriter::new(data_log);
        writer.write_all(merged_data_str.as_bytes()).await?;
        writer.flush().await?;

        let mut writer = BufWriter::new(diff_log);
        // let output = serde_json::to_vec_pretty(&p)?;

        for (idx, mut data) in merged_data.into_iter().enumerate() {
            let pkmn_val = data.remove("pkmn").unwrap();
            let ptcgio_val = data.remove("ptcgio").unwrap();

            let diffset = diff::diff(&ptcgio_val, &pkmn_val).collect_vec();

            if !diffset.is_empty() {
                writer.write_all(format!("Differences exist in idx {} which can be seen in the following\n####################\n\npkmn\n==================\n", idx).as_bytes()).await?;
                let pkmn_str = serde_json::to_string_pretty(&pkmn_val)?;
                writer.write_all(pkmn_str.as_bytes()).await?;
                writer
                    .write_all("\n==================\n\nptcgio\n==================\n".as_bytes())
                    .await?;
                let ptcgio_str = serde_json::to_string_pretty(&ptcgio_val)?;
                writer.write_all(ptcgio_str.as_bytes()).await?;
                writer
                    .write_all("\n==================\n\n".as_bytes())
                    .await?;
            }
            for difference in &diffset {
                let mut output = "Difference between left and right for path \"root".to_string();
                for step in &difference.path {
                    match step {
                        ValueIndex::Number(num) => {
                            output.push('[');
                            output.push_str(&num.to_string());
                            output.push(']');
                        }
                        ValueIndex::Key(key) => {
                            output.push('.');
                            output.push_str(key);
                        }
                    }
                }
                output.push_str("\"\n\n    left: ");
                output.push_str(&difference.left.to_string());
                output.push_str("\n\n    right: ");
                output.push_str(&difference.right.to_string());
                output.push_str("\n\n");

                writer.write_all(output.as_bytes()).await?;
            }
            if !diffset.is_empty() {
                writer
                    .write_all("####################\n".as_bytes())
                    .await?;
            }
            writer.flush().await?;
        }
    }
    Ok(())
}

// fn cmp_cards(card1: &Card, card2: &Card) -> Ordering {
//     match card1.name.cmp(&card2.name) {
//         Ordering::Less => Ordering::Less,
//         Ordering::Equal => card1.number.cmp(&card2.number),
//         Ordering::Greater => Ordering::Greater,
//     }
// }

fn sort_value(val: Value) -> Value {
    match val {
        Value::Null => Value::Null,
        Value::Bool(v) => Value::Bool(v),
        Value::String(v) => Value::String(v),
        Value::Number(v) => Value::Number(v),
        Value::Object(v) => Value::Object(
            v.into_iter()
                .map(|(key, value)| (key, sort_value(value)))
                .collect(),
        ),
        Value::Array(v) => Value::Array(
            v.into_iter()
                .map(sort_value)
                .sorted_by(cmp_values)
                .collect_vec(),
        ),
    }
}

fn cmp_values(val1: &Value, val2: &Value) -> Ordering {
    match (val1, val2) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Bool(v1), Value::Bool(v2)) => v1.cmp(v2),
        (Value::String(v1), Value::String(v2)) => v1.cmp(v2),
        (Value::Number(v1), Value::Number(v2)) => v1.as_i64().unwrap().cmp(&v2.as_i64().unwrap()),
        (Value::Array(v1), Value::Array(v2)) => v1.len().cmp(&v2.len()),
        (Value::Object(v1), Value::Object(v2)) => v1.len().cmp(&v2.len()),
        _ => Ordering::Equal,
    }
}
