mod pkmn_data;
mod ptcgio_data;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    // const PERSONAL_DATA_URL: &str = "https://github.com/ProfDoof/pokemon-tcg-data.git";
    // let data = ptcgio_data::DataFetcher::new(PERSONAL_DATA_URL, "ptcg-data").fetch()?;
    // let filtered_data = data
    //     .sets
    //     .iter()
    //     .flat_map(|set| {
    //         set.cards.iter().filter(|card| {
    //             card.resistances.as_ref().is_some_and(|resistances| {
    //                 resistances.iter().any(|map| {
    //                     map.get("value")
    //                         .is_some_and(|value| value != "-30" && value != "-20")
    //                 })
    //             }) || card.weaknesses.as_ref().is_some_and(|resistances| {
    //                 resistances
    //                     .iter()
    //                     .any(|map| map.get("value").is_some_and(|value| value != "×2"))
    //             })
    //         })
    //     })
    //     .collect::<Vec<&ptcgio_data::Card>>();
    //
    // println!("{:#?}\n{}", filtered_data, filtered_data.len());

    let pkmn_data = pkmn_data::DataFetcher::default().fetch().await?;
    Ok(())
}
