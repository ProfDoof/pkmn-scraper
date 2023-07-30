mod pkmn_data;
mod ptcgio_data;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    // const PERSONAL_DATA_URL: &str = "https://github.com/ProfDoof/pokemon-tcg-data.git";
    // let data = DataFetcher::new(PERSONAL_DATA_URL, "ptcg-data").fetch()?;
    let _pkmn_data = pkmn_data::DataFetcher::default().fetch().await?;
    Ok(())
}
