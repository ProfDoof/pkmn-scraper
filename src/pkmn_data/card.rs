use crate::pkmn_data::extractors::{extract_text, extract_title};
use crate::ptcgio_data::Card;
use anyhow::{anyhow, Context, Error, Result};
use ego_tree::NodeRef;
use regex::Regex;
use reqwest::Client;
use scraper::selector::CssLocalName;
use scraper::{ElementRef, Node, Selector};
use std::collections::HashSet;
use std::iter;
use std::ops::Deref;
use std::str::FromStr;
use strum::EnumString;
use time::macros::format_description;
use time::Date;

pub(super) struct CardFetcher {
    _url: String,
    _client: Client,
}

impl CardFetcher {
    pub(super) fn new(card_ref: ElementRef, client: &Client) -> Self {
        let url = card_ref.value().attr("href").unwrap().to_string();
        log::trace!("url for card: {}", url);
        Self {
            _url: url,
            _client: client.clone(),
        }
    }

    pub(super) async fn fetch(&self) -> Result<Card> {
        let card = Card::default();

        let name_hp_color_selector = Selector::parse("div.name-hp-color");

        Ok(card)
    }
}

trait PkmnParse {
    type Parsed;
    fn parse(element: ElementRef) -> Result<Self::Parsed>;
}

#[derive(Eq, PartialEq, Debug, EnumString)]
enum PokeColor {
    Grass,
    Fire,
    Water,
    Lightning,
    Fighting,
    Psychic,
    Colorless,
    Dark,
    Metal,
    Dragon,
    Fairy,
    #[strum(default)]
    None(String),
}

struct CardText {
    // name-hp-color
    name_hp_color: NameHpColor,

    // type-evolves-is
    type_evolves_is: TypeEvolvesIs,

    // text
    all_text_info: AllTextInfo,

    // weak-resist-retreat
    weak_resist_retreat: WeakResistRetreat,

    // rules
    rules: Option<Rules>,

    // illus
    illus: Illus,

    // release-meta
    release_meta: ReleaseMeta,

    // mark-formats
    mark_formats: MarkFormats,

    // flavor
    flavor_text: Option<String>,
}

struct NameHpColor {
    name: String,
    hp: Option<i32>,
    color: Option<PokeColor>,
}

#[derive(Eq, PartialEq, Debug)]
struct TypeEvolvesIs {
    pkmn_type: PkmnSuperType,
    pkmn_subtype: Option<PkmnSubtype>,
    pkmn_subsubtype: Option<PkmnSubSubType>,
    all_pokemon: Vec<String>,
    stage: Option<Stage>,
    evolves: Option<Evolves>,
    is: HashSet<PtcgTag>,
}

impl PkmnParse for TypeEvolvesIs {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        // Get Pkmn Type
        let pkmn_type_selector = Selector::parse("span.type").unwrap();
        let pkmn_type = PkmnSuperType::from_str(&extract_text(element, pkmn_type_selector)?)?;
        // Get Pkmn Subtype
        let pkmn_subtype_selector = Selector::parse("span.sub-type > a").unwrap();
        let mut pkmn_subtype_iter = element.select(&pkmn_subtype_selector);
        let (pkmn_subtype, pkmn_subsubtype) = if let Some(subtype) = pkmn_subtype_iter.next() {
            (
                Some(
                    PkmnSubtype::from_str(&subtype.text().collect::<String>())
                        .with_context(|| format!("Error getting subtype: {}", subtype.html()))?,
                ),
                pkmn_subtype_iter
                    .next()
                    .map(|subsubtype| {
                        PkmnSubSubType::from_str(&subsubtype.text().collect::<String>())
                    })
                    .transpose()
                    .with_context(|| format!("Error getting subsubtype: {}", element.html()))?,
            )
        } else {
            (None, None)
        };

        // Get Pokemons
        let pokemon_selector = Selector::parse("span.pokemons > span.pokemon").unwrap();
        let all_pokemon = element
            .select(&pokemon_selector)
            .map(|elem| elem.text().collect::<String>())
            .collect::<Vec<String>>();

        // Get Stage
        let stage_selector = Selector::parse("span.stage").unwrap();
        let stage = element
            .select(&stage_selector)
            .next()
            .map(|elem| Stage::from_str(&elem.text().collect::<String>()))
            .transpose()?;

        // Get Evolves
        let evolves_selector = Selector::parse("span.evolves").unwrap();
        let evolves = element
            .select(&evolves_selector)
            .next()
            .map(|elem| Evolves::parse(elem))
            .transpose()?;

        // Get Is
        let is_selector = Selector::parse("span.is > a").unwrap();
        let is = element
            .select(&is_selector)
            .map(
                |elem| match PtcgTag::from_str(&elem.text().collect::<String>()) {
                    Ok(tag) => Ok(tag),
                    Err(e) => Err(anyhow!(e)),
                },
            )
            .collect::<Result<HashSet<PtcgTag>>>()?;

        Ok(TypeEvolvesIs {
            pkmn_type,
            pkmn_subtype,
            pkmn_subsubtype,
            all_pokemon,
            stage,
            evolves,
            is,
        })
    }
}

#[derive(Eq, PartialEq, Debug)]
struct AllTextInfo {
    text_infos: Vec<TextInfo>,
}

impl PkmnParse for AllTextInfo {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let text_info_selector = Selector::parse("div.text > p").unwrap();
        Ok(AllTextInfo {
            text_infos: element
                .select(&text_info_selector)
                .map(TextInfo::parse)
                .collect::<Result<_>>()?,
        })
    }
}

#[derive(Eq, PartialEq, Debug)]
struct WeakResistRetreat {
    weak: PokeColor,
    resist: PokeColor,
    retreat: i32,
}

impl PkmnParse for WeakResistRetreat {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        // get weak
        let weak_selector = Selector::parse("span.weak > a > abbr").unwrap();
        let weak = PokeColor::from_str(&extract_title(element, weak_selector)?)?;

        // get resist
        let resist_selector = Selector::parse("span.resist > a > abbr").unwrap();
        let resist = PokeColor::from_str(&extract_title(element, resist_selector)?)?;

        // get retreat
        let retreat_selector = Selector::parse("span.retreat > a > abbr").unwrap();
        let retreat = extract_text(element, retreat_selector)?.parse::<i32>()?;

        Ok(WeakResistRetreat {
            weak,
            resist,
            retreat,
        })
    }
}

struct Rules {
    rules: Vec<Rule>,
}

impl PkmnParse for Rules {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let rule_selector = Selector::parse("div.rule").unwrap();
        let rules = element
            .select(&rule_selector)
            .map(Rule::parse)
            .collect::<Result<_>>()?;

        Ok(Rules { rules })
    }
}

struct Illus {
    illustrator: String,
    level: Option<String>,
}

impl PkmnParse for Illus {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let illustrator_selector =
            Selector::parse("span[title=\"Illustrator\"] > a[title=\"Illustrator\"]").unwrap();
        let illustrator = extract_text(element, illustrator_selector)?;

        let level_selector = Selector::parse("span.level > a").unwrap();
        let level = element
            .select(&level_selector)
            .next()
            .map(|level| level.text().collect::<String>());

        Ok(Illus { illustrator, level })
    }
}

#[derive(Eq, PartialEq, Debug)]
enum SetNumber {
    Num(i32),
    Str(String),
}

impl FromStr for SetNumber {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(s.parse::<i32>()
            .map(SetNumber::Num)
            .map_err(|_| SetNumber::Str(s.to_string()))
            .unwrap())
    }
}

#[derive(Eq, PartialEq, Debug)]
struct ReleaseMeta {
    series: Vec<String>,
    set: String,
    set_abbreviation: String,
    set_series_code: String,
    set_number: SetNumber,
    set_total_cards: Option<i32>,
    rarity: String,
    date_released: Date,
}

impl PkmnParse for ReleaseMeta {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        // Get Series
        let series_selector =
            Selector::parse("span[title=\"Series\"] > a[title=\"Series\"]").unwrap();
        let series = element
            .select(&series_selector)
            .map(|series| series.text().collect::<String>())
            .collect();

        // Get Set Name
        let set_selector = Selector::parse("span[title=\"Set\"] > a").unwrap();
        let set = extract_text(element, set_selector)?;

        // Get Set Abbreviation
        let set_abbr_selector = Selector::parse("span[title=\"Set Abbreviation\"]").unwrap();
        let set_abbreviation = extract_text(element, set_abbr_selector)?;

        // Get Set Code
        let set_series_code_selector = Selector::parse("span[title=\"Set Series Code\"]").unwrap();
        let set_series_code = extract_text(element, set_series_code_selector)?;

        // Get Set Number
        let set_number_selector = Selector::parse("span.number-out-of > span.number").unwrap();
        let set_number = SetNumber::from_str(&extract_text(element, set_number_selector)?)?;

        // Get Total Cards in Set if available
        let set_total_cards_selector = Selector::parse("span.number-out-of > span.out-of").unwrap();
        let set_total_cards = element
            .select(&set_total_cards_selector)
            .next()
            .map(|elem| {
                let str = elem.text().collect::<String>();
                str.parse::<i32>()
            })
            .transpose()?;

        // Get Rarity
        let rarity_selector = Selector::parse("span.rarity > a[title=\"Rarity\"]").unwrap();
        let rarity = extract_text(element, rarity_selector)?;

        // Get Date Released
        let date_released_selector = Selector::parse("span.date[title=\"Date Released\"]").unwrap();
        let format_description = format_description!(
            version = 2,
            "↘ [month repr:short] [day padding:none], [year]"
        );

        let date_released = Date::parse(
            &extract_text(element, date_released_selector)?,
            format_description,
        )?;

        Ok(ReleaseMeta {
            series,
            set,
            set_abbreviation,
            set_series_code,
            set_number,
            set_total_cards,
            rarity,
            date_released,
        })
    }
}

#[derive(Eq, PartialEq, Debug)]
struct MarkFormats {
    mark: Option<Mark>,
    formats: Vec<Formats>,
}

impl PkmnParse for MarkFormats {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let mark_selector = Selector::parse("span.Regulation.Mark > a").unwrap();
        let format_selector = Selector::parse("span[title=\"Format Type\"]").unwrap();
        let mark = element
            .select(&mark_selector)
            .next()
            .map(|mark| {
                let mark_str = mark.text().collect::<String>();
                println!("{}", mark_str);
                Mark::from_str(&mark_str)
            })
            .transpose()?;
        let formats = element
            .select(&format_selector)
            .map(Formats::parse)
            .collect::<Result<Vec<Formats>>>()?;

        Ok(MarkFormats { mark, formats })
    }
}

#[derive(Eq, PartialEq, Debug, EnumString)]
enum PkmnSuperType {
    #[strum(serialize = "Pokémon")]
    Pokemon,
    Trainer,
    Energy,
}

#[derive(Eq, PartialEq, Debug, EnumString)]
enum PkmnSubtype {
    Item,
    Supporter,
    #[strum(serialize = "Basic Energy")]
    BasicEnergy,
    Tool,
    Stadium,
    #[strum(serialize = "Special Energy")]
    SpecialEnergy,
    /// No longer considered a sub sub type as tools are no longer subtypes of items
    #[strum(serialize = "Pokémon Tool F")]
    PokemonToolF,
}

#[derive(Eq, PartialEq, Debug, EnumString)]
enum PkmnSubSubType {
    #[strum(serialize = "Technical Machine")]
    TechnicalMachine,
    #[strum(serialize = "Rocket's Secret Machine")]
    RocketsSecretMachine,
    #[strum(serialize = "Goldenrod Game Corner")]
    GoldenrodGameCorner,
}

#[derive(Eq, PartialEq, Debug, EnumString)]
enum Stage {
    Basic,
    #[strum(serialize = "Stage 1")]
    Stage1,
    #[strum(serialize = "Stage 2")]
    Stage2,
    #[strum(serialize = "VMAX")]
    Vmax,
    #[strum(serialize = "VSTAR")]
    Vstar,
    Mega,
    #[strum(serialize = "Level-Up")]
    LevelUp,
    #[strum(serialize = "BREAK")]
    Break,
    #[strum(serialize = "V-UNION")]
    VUnion,
    Baby,
    #[strum(serialize = "LEGEND")]
    Legend,
    Restored,
}

#[derive(Eq, PartialEq, Debug)]
struct Evolves {
    from: Vec<String>,
    to: Vec<String>,
}

impl PkmnParse for Evolves {
    type Parsed = Evolves;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let from_to_re =
            Regex::new(r#"Evolves (from (?<from>(.*?)))?( and )?(into (?<to>(.*?)))?$"#).unwrap();
        let text = element.text().collect::<String>();
        let caps = from_to_re.captures(&text).ok_or(Error::msg(format!(
            "Could not extract the evolves to and from: {}",
            element.html()
        )))?;

        let splitter_re = Regex::new(r#"((\s*or\s*)|(\s*,\s*))"#)?;
        let from: Vec<String> = if let Some(cap) = caps.name("from") {
            splitter_re
                .split(cap.as_str())
                .filter_map(|mon| {
                    if mon.is_empty() {
                        None
                    } else {
                        Some(mon.to_string())
                    }
                })
                .collect()
        } else {
            Vec::with_capacity(0)
        };

        let to: Vec<String> = if let Some(cap) = caps.name("to") {
            splitter_re
                .split(cap.as_str())
                .filter_map(|mon| {
                    if mon.is_empty() {
                        None
                    } else {
                        Some(mon.to_string())
                    }
                })
                .collect()
        } else {
            Vec::with_capacity(0)
        };

        Ok(Evolves { from, to })
    }
}

#[derive(Eq, PartialEq, Debug, Hash, EnumString)]
#[strum(serialize_all = "kebab-case")]
enum PtcgTag {
    V,
    GX,
    #[strum(serialize = "ex-%e2%86%91")]
    ExUpper,
    DeltaSpecies,
    RapidStrike,
    #[strum(serialize = "ex-%e2%86%93")]
    ExLower,
    SingleStrike,
    Galarian,
    TagTeam,
    Dynamax,
    Dark,
    TeamPlasma,
    Ball,
    Alolan,
    SP,
    UltraBeast,
    DualType,
    #[strum(serialize = "ex-3")]
    Ex3,
    Gigantamax,
    Hisuian,
    FusionStrike,
    Fossil,
    TeamAquas,
    TeamMagmas,
    G,
    Prime,
    Star,
    Brocks,
    TeamRockets,
    Sabrinas,
    PrismStar,
    Erikas,
    Mistys,
    Holon,
    Blaines,
    E4,
    LtSurges,
    Light,
    GL,
    Shining,
    ScoopUp,
    Berry,
    Kogas,
    Radiant,
    Potion,
    C,
    Giovannis,
    FB,
    AceSpec,
    Rod,
    Crystal,
    Tera,
    Gloves,
    Paldean,
    Lucky,
    Primal,
    Shard,
    Plate,
    Board,
    Eternamax,
    Sphere,
    Plus,
    Broken,
    Lances,
    Imakunis,
    Cool,
}

#[derive(Eq, PartialEq, Debug)]
enum TextInfo {
    Ability {
        name: String,
        text: String,
    },
    PokeBody {
        name: String,
        text: String,
    },
    PokePower {
        name: String,
        text: String,
    },
    PokemonPower {
        name: String,
        text: String,
    },
    AncientTrait {
        name: String,
        text: String,
    },
    HeldItem {
        name: String,
        text: String,
    },
    Attack {
        cost: Vec<EnergyColor>,
        name: String,
        damage: Option<String>,
        text: String,
    },
}

impl TextInfo {
    fn make_ability(
        ability_type: &str,
        ability_name: String,
        ability_text: String,
    ) -> Result<Self> {
        Ok(match ability_type {
            "Ability" => TextInfo::Ability {
                name: ability_name,
                text: ability_text,
            },
            "Poké-BODY" => TextInfo::PokeBody {
                name: ability_name,
                text: ability_text,
            },
            "Poké-POWER" => TextInfo::PokePower {
                name: ability_name,
                text: ability_text,
            },
            "Pokémon Power" => TextInfo::PokemonPower {
                name: ability_name,
                text: ability_text,
            },
            "Ancient Trait" => TextInfo::AncientTrait {
                name: ability_name,
                text: ability_text,
            },
            "Held Item" => TextInfo::HeldItem {
                name: ability_name,
                text: ability_text,
            },
            _ => Err(Error::msg(format!(
                "Unknown ability type: {}",
                ability_type
            )))?,
        })
    }

    fn get_text(element: ElementRef) -> Result<String, Error> {
        Ok(element
            .children()
            .skip_while(is_not_break)
            .map(read_text)
            .collect::<Result<Vec<Box<dyn Iterator<Item = &str>>>>>()?
            .into_iter()
            .flatten()
            .collect::<String>()
            .trim()
            .to_string())
    }

    fn get_string_til_break(element: ElementRef) -> Result<String, Error> {
        Ok({
            let test = element
                .next_siblings()
                .map_while(read_text_til_break)
                .collect::<Result<Vec<Box<dyn Iterator<Item = &str>>>>>()?
                .into_iter()
                .flatten()
                .collect::<String>();
            println!("test: {}", test);
            test.trim_start_matches([' ', '⇢', '→', '{', '}', '+'])
                .trim()
                .to_string()
        })
    }

    fn get_cost(element: ElementRef) -> Result<(Option<ElementRef>, Vec<EnergyColor>), Error> {
        let energy_and_br_selector =
            Selector::parse("abbr.ptcg-font.ptcg-symbol-name, br").unwrap();

        let mut last_energy = None;
        let cost = element
            .select(&energy_and_br_selector)
            .map_while(|element| {
                if element.value().name.local == CssLocalName::from("br").0 {
                    None
                } else {
                    last_energy = Some(element);
                    Some(EnergyColor::from_str(
                        element.value().attr("title").unwrap(),
                    ))
                }
            })
            .collect::<Result<Vec<EnergyColor>, strum::ParseError>>()?;
        Ok((last_energy, cost))
    }

    fn get_name_and_damage(
        html: String,
        last_energy: Option<ElementRef>,
    ) -> Result<(String, Option<String>), Error> {
        let name_and_damage = Self::get_string_til_break(
            last_energy.ok_or(anyhow!("Failed to extract name from: {}", html))?,
        )?;

        let re = Regex::new(r#"^(?<name>.*?)(:\s*(?<damage>.*?)\s*)?$"#).unwrap();
        let captures = re.captures(&name_and_damage).ok_or(Error::msg(format!(
            "Could not extract name or damage from: {}",
            html
        )))?;
        let name = captures
            .name("name")
            .ok_or(anyhow!("Could not extract name from: {}", html))?
            .as_str()
            .trim()
            .to_string();
        let damage = captures.name("damage").map(|dmg| dmg.as_str().to_string());
        Ok((name, damage))
    }
}

fn read_text_til_break(
    node_ref: NodeRef<Node>,
) -> Option<Result<Box<dyn Iterator<Item = &str> + '_>>> {
    if is_not_break(&node_ref) {
        Some(read_text(node_ref))
    } else {
        None
    }
}
//

fn is_not_break(node_ref: &NodeRef<Node>) -> bool {
    let wrapped = ElementRef::wrap(*node_ref);
    let break_name = CssLocalName::from("br").0;
    if let Some(element) = wrapped {
        println!("{}", element.value().name.local);
        element.value().name.local != break_name
    } else {
        true
    }
}

//
fn read_text(node_ref: NodeRef<Node>) -> Result<Box<dyn Iterator<Item = &str> + '_>> {
    let wrapped = ElementRef::wrap(node_ref);
    if let Some(element) = wrapped {
        Ok(Box::new(element.text()))
    } else {
        match node_ref.value() {
            Node::Comment(_) => Ok(Box::new(iter::empty())),
            Node::Text(text) => Ok(Box::new(iter::once(text.deref()))),
            Node::ProcessingInstruction(_) => Ok(Box::new(iter::empty())),
            _ => Err(Error::msg("Unknown node type")),
        }
    }
}

impl PkmnParse for TextInfo {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let html = element.html();
        let mut children = element.children();
        let discriminator = ElementRef::wrap(
            children
                .next()
                .ok_or(anyhow!("No discriminator available: {}", html))?,
        )
        .unwrap();
        Ok({
            let local_name = &discriminator.value().name.local;
            if local_name == &CssLocalName::from("a").0 {
                let ability_type = discriminator.inner_html();
                let ability_name = Self::get_string_til_break(discriminator)?;
                let ability_text = Self::get_text(element)?;
                TextInfo::make_ability(ability_type.as_str(), ability_name, ability_text)?
            } else {
                let (last_energy, cost) = Self::get_cost(element)?;
                let (name, damage) = Self::get_name_and_damage(html, last_energy)?;
                let text = Self::get_text(element)?;
                TextInfo::Attack {
                    cost,
                    name,
                    damage,
                    text,
                }
            }
        })
    }
}

#[derive(Eq, PartialEq, Debug)]
struct Rule {
    purpose: String,
    text: String,
}

impl PkmnParse for Rule {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let purpose_selector = Selector::parse("em").unwrap();
        let purpose = element
            .select(&purpose_selector)
            .next()
            .ok_or(anyhow!("No purpose found: {}", element.html()))?
            .text()
            .next()
            .ok_or(anyhow!("No purpose found: {}", element.html()))?
            .trim()
            .to_string();

        let mut element_text = element.text().skip_while(|text| !text.contains(':'));
        element_text.next();
        let rule_text: String = element_text.collect();
        Ok(Rule {
            purpose,
            text: rule_text.trim().to_string(),
        })
    }
}

#[derive(Eq, PartialEq, Debug, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
enum Mark {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

#[derive(Eq, PartialEq, Debug)]
struct Formats {
    format: FormatType,
    formats: Vec<PtcgFormat>,
}

impl PkmnParse for Formats {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let format_selector = Selector::parse("a").unwrap();
        let format_type = FormatType::from_str(element.text().next().unwrap())?;
        let formats = element
            .select(&format_selector)
            .map(PtcgFormat::parse)
            .collect::<Result<_>>()?;
        Ok(Formats {
            format: format_type,
            formats,
        })
    }
}

#[derive(Eq, PartialEq, Debug, EnumString)]
enum FormatType {
    #[strum(serialize = "Standard: ")]
    Standard,
    #[strum(serialize = "Expanded: ")]
    Expanded,
    #[strum(serialize = "Modified: ")]
    Modified,
    #[strum(serialize = "Other: ")]
    Other,
}

#[derive(Eq, PartialEq, Debug)]
struct PtcgFormat {
    id: String,
    text: String,
}

impl PkmnParse for PtcgFormat {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let html = element.html();
        let id = element
            .value()
            .attr("title")
            .ok_or(anyhow!("Format did not have title: {}", html))?
            .to_string();
        let text: String = element.text().collect();
        Ok(PtcgFormat { id, text })
    }
}

#[derive(Eq, PartialEq, Debug, EnumString)]
enum EnergyColor {
    Grass,
    Fire,
    Water,
    Lightning,
    Psychic,
    Fighting,
    Dark,
    Metal,
    Fairy,
    Colorless,
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;
    use time::Month;

    // #[test]
    // fn test() {
    //     let root = Selector::parse("html").unwrap();
    //     let test = Selector::parse(":scope > p").unwrap();
    //     let test_fragment = Html::parse_fragment(
    //         r#"
    //     <p title="top">
    //         <p title="inner">ree</p>
    //     </p>"#,
    //     );
    //     let start = test_fragment.select(&root).next().unwrap();
    //     for elem in start
    //         .first_children()
    //         .flat_map(ElementRef::wrap)
    //         .filter(|elem| test.matches_with_scope(elem, Some(start)))
    //     {
    //         println!("Element {:?}", elem);
    //     }
    //     panic!()
    // }

    #[test]
    fn parse_type_evolve_is() {
        let fragment = Html::parse_fragment(
            r#"<div class="type-evolves-is"><span class="type" title="Type"><a href="https://pkmncards.com/type/pokemon/">Pokémon</a></span> <span class="pokemons">(<span class="pokemon" title="Pokémon"><a href="https://pkmncards.com/pokemon/dragonair/">Dragonair</a></span>)</span> › <span class="stage" title="Stage of Evolution"><a href="https://pkmncards.com/stage/stage-1/">Stage 1</a></span> : <span class="evolves">Evolves from <a href="https://pkmncards.com/name/dratini/" title="Name">Dratini</a> and into <a href="https://pkmncards.com/name/dragonite/" title="Name">Dragonite</a>, <a href="https://pkmncards.com/name/dragonite-gx/" title="Name">Dragonite-<em>GX</em></a>, or <a href="https://pkmncards.com/name/dragonite-ex-%e2%86%93/" title="Name">Dragonite ex</a></span></div>"#,
        );
        let selector = Selector::parse("div").unwrap();
        let actual = TypeEvolvesIs::parse(fragment.select(&selector).next().unwrap()).unwrap();
        let expected = TypeEvolvesIs {
            pkmn_type: PkmnSuperType::Pokemon,
            pkmn_subtype: None,
            pkmn_subsubtype: None,
            all_pokemon: vec!["Dragonair".to_string()],
            stage: Some(Stage::Stage1),
            evolves: Some(Evolves {
                from: vec!["Dratini".to_string()],
                to: vec![
                    "Dragonite".to_string(),
                    "Dragonite-GX".to_string(),
                    "Dragonite ex".to_string(),
                ],
            }),
            is: HashSet::default(),
        };

        assert_eq!(actual, expected);

        let fragment = Html::parse_fragment(
            r#"<div class="type-evolves-is"><span class="type" title="Type"><a href="https://pkmncards.com/type/trainer/">Trainer</a></span> › <span class="sub-type" title="Sub-Type">(<a href="https://pkmncards.com/type/item/">Item</a>)</span> › <span class="sub-type" title="Sub-Type"><a href="https://pkmncards.com/type/rockets-secret-machine/">Rocket's Secret Machine</a></span></div>"#,
        );
        let actual = TypeEvolvesIs::parse(fragment.select(&selector).next().unwrap()).unwrap();
        let expected = TypeEvolvesIs {
            pkmn_type: PkmnSuperType::Trainer,
            pkmn_subtype: Some(PkmnSubtype::Item),
            pkmn_subsubtype: Some(PkmnSubSubType::RocketsSecretMachine),
            all_pokemon: vec![],
            stage: None,
            evolves: None,
            is: Default::default(),
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_all_text_info() {
        let fragment = Html::parse_fragment(
            r#"<div class="text"><p><abbr title="Fighting" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>F<span class="vh">}</span></abbr> → <span>Angry Grudge</span> : 20×<br>
Put up to 12 damage counters on this Pokémon. This attack does 20 damage for each damage counter you placed in this way.</p>
<p><abbr title="Fighting" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>F<span class="vh">}</span></abbr><abbr title="Colorless" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>C<span class="vh">}</span></abbr> → <span>Seismic Toss</span> : 150</p>
</div>"#,
        );
        let selector = Selector::parse("div").unwrap();
        let actual = AllTextInfo::parse(fragment.select(&selector).next().unwrap()).unwrap();
        let expected = AllTextInfo {
            text_infos: vec![
                TextInfo::Attack {
                    cost: vec![EnergyColor::Fighting],
                    name: "Angry Grudge".to_string(),
                    damage: Some("20×".to_string()),
                    text: "Put up to 12 damage counters on this Pokémon. This attack does 20 damage for each damage counter you placed in this way.".to_string(),
                },
                TextInfo::Attack {
                    cost: vec![EnergyColor::Fighting, EnergyColor::Colorless],
                    name: "Seismic Toss".to_string(),
                    damage: Some("150".to_string()),
                    text: "".to_string(),
                },
            ],
        };

        assert_eq!(actual, expected)
    }

    #[test]
    fn parse_weak_resist_retreat() {
        let fragment = Html::parse_fragment(
            r#"<div class="weak-resist-retreat"><span class="weak" title="Weakness">weak: <a href="https://pkmncards.com/weakness/psychic/"><abbr title="Psychic" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>P<span class="vh">}</span></abbr></a><span title="Weakness Modifier">×2</span></span> | <span class="resist" title="Resistance">resist: <a href="https://pkmncards.com/?s=-resist%3A%2A"><abbr title="No Resistance">n/a</abbr></a></span> | <span class="retreat" title="Retreat Cost">retreat: <a href="https://pkmncards.com/retreat-cost/2/"><abbr title="{C}{C}">2</abbr></a></span></div>"#,
        );
        let selector = Selector::parse("div").unwrap();
        let actual = WeakResistRetreat::parse(fragment.select(&selector).next().unwrap()).unwrap();
        let expected = WeakResistRetreat {
            weak: PokeColor::Psychic,
            resist: PokeColor::None("No Resistance".to_string()),
            retreat: 2,
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_release_meta() {
        let fragment = Html::parse_fragment(
            r#"<div class="release-meta minor-text"><span title="Series"><a href="https://pkmncards.com/series/promos/" title="Series">Promos</a>, <a href="https://pkmncards.com/series/scarlet-violet/" title="Series">Scarlet &amp; Violet</a></span> › <span title="Set"><a href="https://pkmncards.com/set/scarlet-violet-promos/">Scarlet &amp; Violet Promos</a></span> (<span title="Set Abbreviation">SVP</span>, <span title="Set Series Code">Promo_SV</span>) › <span class="number-out-of">#<span class="number"><a href="https://pkmncards.com/number/032/" title="Number">032</a></span></span> : <span class="rarity"><a href="https://pkmncards.com/rarity/promo/" title="Rarity">Promo</a></span> · <span class="date" title="Date Released">↘ Jul 14, 2023</span></div>"#,
        );
        let selector = Selector::parse("div").unwrap();
        let actual = ReleaseMeta::parse(fragment.select(&selector).next().unwrap()).unwrap();
        let expected = ReleaseMeta {
            series: vec!["Promos".to_string(), "Scarlet & Violet".to_string()],
            set: "Scarlet & Violet Promos".to_string(),
            set_abbreviation: "SVP".to_string(),
            set_series_code: "Promo_SV".to_string(),
            set_number: SetNumber::Num(32),
            set_total_cards: None,
            rarity: "Promo".to_string(),
            date_released: Date::from_calendar_date(2023, Month::July, 14).unwrap(),
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_mark_formats() {
        let fragment = Html::parse_fragment(
            r#"<div class="mark-formats minor-text"><span class="Regulation Mark">Mark: <a href="https://pkmncards.com/regulation-mark/g/">G</a></span> · <span title="Legal Formats">Formats: <span title="Format Type">Standard: <a href="https://pkmncards.com/format/e-on-standard-2024/" title="2024">E–on</a></span> · <span title="Format Type">Expanded: <a href="https://pkmncards.com/format/blw-on-expanded-current/" title="BLW–on">Current</a></span></span></div>"#,
        );
        let mark_format_selector = Selector::parse("div").unwrap();
        let actual =
            MarkFormats::parse(fragment.select(&mark_format_selector).next().unwrap()).unwrap();
        let expected = MarkFormats {
            mark: Some(Mark::G),
            formats: vec![
                Formats {
                    format: FormatType::Standard,
                    formats: vec![PtcgFormat {
                        id: "2024".to_string(),
                        text: "E–on".to_string(),
                    }],
                },
                Formats {
                    format: FormatType::Expanded,
                    formats: vec![PtcgFormat {
                        id: "BLW–on".to_string(),
                        text: "Current".to_string(),
                    }],
                },
            ],
        };

        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_evolves() {
        let fragment = Html::parse_fragment(
            r#"<span class="evolves">Evolves from <a href="https://pkmncards.com/name/tinkatink/" title="Name">Tinkatink</a> and into <a href="https://pkmncards.com/name/tinkaton-ex-%e2%86%93/" title="Name">Tinkaton ex</a> or <a href="https://pkmncards.com/name/tinkaton/" title="Name">Tinkaton</a> or Riemann</span>"#,
        );
        let evolve_selector = Selector::parse("span.evolves").unwrap();

        let parsed = Evolves::parse(fragment.select(&evolve_selector).next().unwrap()).unwrap();
        let expected = Evolves {
            from: vec!["Tinkatink".to_string()],
            to: vec![
                "Tinkaton ex".to_string(),
                "Tinkaton".to_string(),
                "Riemann".to_string(),
            ],
        };

        assert_eq!(parsed, expected);
    }

    #[test]
    fn parse_ptcg_tag() {
        let fragment = Html::parse_fragment(
            r#"<div class="form-row-content">
					<label class="checkbox"><input type="checkbox" name="is[]" value="v">V</label> <label class="checkbox"><input type="checkbox" name="is[]" value="gx">GX</label> <label class="checkbox"><input type="checkbox" name="is[]" value="ex-%e2%86%91">EX</label> <label class="checkbox"><input type="checkbox" name="is[]" value="delta-species">Delta Species</label> <label class="checkbox"><input type="checkbox" name="is[]" value="rapid-strike">Rapid Strike</label> <label class="checkbox"><input type="checkbox" name="is[]" value="ex-%e2%86%93">ex</label> <label class="checkbox"><input type="checkbox" name="is[]" value="single-strike">Single Strike</label> <label class="checkbox"><input type="checkbox" name="is[]" value="galarian">Galarian</label> <label class="checkbox"><input type="checkbox" name="is[]" value="tag-team">TAG TEAM</label> <label class="checkbox"><input type="checkbox" name="is[]" value="dynamax">Dynamax</label> <label class="checkbox"><input type="checkbox" name="is[]" value="dark">Dark</label> <label class="checkbox"><input type="checkbox" name="is[]" value="team-plasma">Team Plasma</label> <label class="checkbox"><input type="checkbox" name="is[]" value="ball">Ball</label> <label class="checkbox"><input type="checkbox" name="is[]" value="alolan">Alolan</label> <label class="checkbox"><input type="checkbox" name="is[]" value="sp">SP</label> <label class="checkbox"><input type="checkbox" name="is[]" value="ultra-beast">Ultra Beast</label> <label class="checkbox"><input type="checkbox" name="is[]" value="dual-type">Dual Type</label> <label class="checkbox"><input type="checkbox" name="is[]" value="ex-3">ex</label> <label class="checkbox"><input type="checkbox" name="is[]" value="gigantamax">Gigantamax</label> <label class="checkbox"><input type="checkbox" name="is[]" value="hisuian">Hisuian</label> <label class="checkbox"><input type="checkbox" name="is[]" value="fusion-strike">Fusion Strike</label> <label class="checkbox"><input type="checkbox" name="is[]" value="fossil">Fossil</label> <label class="checkbox"><input type="checkbox" name="is[]" value="team-aquas">Team Aqua's</label> <label class="checkbox"><input type="checkbox" name="is[]" value="team-magmas">Team Magma's</label> <label class="checkbox"><input type="checkbox" name="is[]" value="g">G</label> <label class="checkbox"><input type="checkbox" name="is[]" value="prime">Prime</label> <label class="checkbox"><input type="checkbox" name="is[]" value="star">Star</label> <label class="checkbox"><input type="checkbox" name="is[]" value="brocks">Brock's</label> <label class="checkbox"><input type="checkbox" name="is[]" value="team-rockets">Team Rocket's</label> <label class="checkbox"><input type="checkbox" name="is[]" value="sabrinas">Sabrina's</label> <label class="checkbox"><input type="checkbox" name="is[]" value="prism-star">Prism Star</label> <label class="checkbox"><input type="checkbox" name="is[]" value="erikas">Erika's</label> <label class="checkbox"><input type="checkbox" name="is[]" value="mistys">Misty's</label> <label class="checkbox"><input type="checkbox" name="is[]" value="holon">Holon</label> <label class="checkbox"><input type="checkbox" name="is[]" value="blaines">Blaine's</label> <label class="checkbox"><input type="checkbox" name="is[]" value="e4">E4</label> <label class="checkbox"><input type="checkbox" name="is[]" value="lt-surges">Lt. Surge's</label> <label class="checkbox"><input type="checkbox" name="is[]" value="light">Light</label> <label class="checkbox"><input type="checkbox" name="is[]" value="gl">GL</label> <label class="checkbox"><input type="checkbox" name="is[]" value="shining">Shining</label> <label class="checkbox"><input type="checkbox" name="is[]" value="scoop-up">Scoop Up</label> <label class="checkbox"><input type="checkbox" name="is[]" value="berry">Berry</label> <label class="checkbox"><input type="checkbox" name="is[]" value="kogas">Koga's</label> <label class="checkbox"><input type="checkbox" name="is[]" value="radiant">Radiant</label> <label class="checkbox"><input type="checkbox" name="is[]" value="potion">Potion</label> <label class="checkbox"><input type="checkbox" name="is[]" value="c">C</label> <label class="checkbox"><input type="checkbox" name="is[]" value="giovannis">Giovanni's</label> <label class="checkbox"><input type="checkbox" name="is[]" value="fb">FB</label> <label class="checkbox"><input type="checkbox" name="is[]" value="ace-spec">ACE SPEC</label> <label class="checkbox"><input type="checkbox" name="is[]" value="rod">Rod</label> <label class="checkbox"><input type="checkbox" name="is[]" value="crystal">Crystal</label> <label class="checkbox"><input type="checkbox" name="is[]" value="tera">Tera</label> <label class="checkbox"><input type="checkbox" name="is[]" value="gloves">Gloves</label> <label class="checkbox"><input type="checkbox" name="is[]" value="paldean">Paldean</label> <label class="checkbox"><input type="checkbox" name="is[]" value="lucky">Lucky</label> <label class="checkbox"><input type="checkbox" name="is[]" value="primal">Primal</label> <label class="checkbox"><input type="checkbox" name="is[]" value="shard">Shard</label> <label class="checkbox"><input type="checkbox" name="is[]" value="plate">Plate</label> <label class="checkbox"><input type="checkbox" name="is[]" value="board">Board</label> <label class="checkbox"><input type="checkbox" name="is[]" value="eternamax">Eternamax</label> <label class="checkbox"><input type="checkbox" name="is[]" value="sphere">Sphere</label> <label class="checkbox"><input type="checkbox" name="is[]" value="plus">+</label> <label class="checkbox"><input type="checkbox" name="is[]" value="broken">Broken</label> <label class="checkbox"><input type="checkbox" name="is[]" value="lances">Lance's</label> <label class="checkbox"><input type="checkbox" name="is[]" value="imakunis">Imakuni?'s</label> <label class="checkbox"><input type="checkbox" name="is[]" value="cool">Cool</label>
				</div>"#,
        );

        let checkbox_selector = Selector::parse("label > input").unwrap();
        fragment
            .select(&checkbox_selector)
            .map(|element| {
                let val = element
                    .value()
                    .attr("value")
                    .ok_or(Error::msg("The checkbox did not contain a value"))?;
                match PtcgTag::from_str(val) {
                    Ok(tag) => Ok(tag),
                    Err(e) => Err(anyhow!(e)),
                }
            })
            .collect::<Result<Vec<PtcgTag>>>()
            .expect("An error occurred while parsing a ptcg tag");
    }

    fn parse_text_info(expected: TextInfo, html: Html) {
        let selector = Selector::parse("p").unwrap();
        let actual = TextInfo::parse(html.select(&selector).next().unwrap()).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_ability() {
        let fragment = Html::parse_fragment(
            r#"<p><a href="https://pkmncards.com/has/ability/">Ability</a> ⇢ Perfection<br>
This Pokémon can use the attacks of any Pokémon-<em>GX</em> or Pokémon-<em>EX</em> on your Bench or in your discard pile. <em>(You still need the necessary Energy to use each attack.)</em></p>"#,
        );

        let expected = TextInfo::Ability {
            name: "Perfection".to_string(),
            text: "This Pokémon can use the attacks of any Pokémon-GX or Pokémon-EX on your Bench or in your discard pile. (You still need the necessary Energy to use each attack.)".to_string(),
        };

        parse_text_info(expected, fragment);
    }

    #[test]
    fn parse_poke_body() {
        let fragment = Html::parse_fragment(
            r#"<p><a href="https://pkmncards.com/has/poke-body/">Poké-BODY</a> ⇢ Exoskeleton<br>
Any damage done to Donphan by attacks is reduced by 20 <em>(after applying Weakness and Resistance)</em>.</p>"#,
        );

        let expected = TextInfo::PokeBody {
            name: "Exoskeleton".to_string(),
            text: "Any damage done to Donphan by attacks is reduced by 20 (after applying Weakness and Resistance).".to_string(),
        };

        parse_text_info(expected, fragment);
    }

    #[test]
    fn parse_poke_power() {
        let fragment = Html::parse_fragment(
            r#"<p><a href="https://pkmncards.com/has/poke-power/">Poké-POWER</a> ⇢ Shadow Knife<br>
When you play this Pokémon from your hand onto your Bench during your turn, you may put 1 damage counter on 1 of your opponent’s Pokémon.</p>"#,
        );
        let expected = TextInfo::PokePower {
            name: "Shadow Knife".to_string(),
            text: "When you play this Pokémon from your hand onto your Bench during your turn, you may put 1 damage counter on 1 of your opponent’s Pokémon.".to_string(),
        };

        parse_text_info(expected, fragment);
    }

    #[test]
    fn parse_pokemon_power() {
        let fragment = Html::parse_fragment(
            r#"<p><a href="https://pkmncards.com/has/pokemon-power/">Pokémon Power</a> ⇢ Energy Burn<br>
As often as you like during your turn <em>(before your attack)</em>, you may turn all Energy attached to Charizard into <abbr title="Fire" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>R<span class="vh">}</span></abbr> Energy for the rest of the turn. This power can’t be used if Charizard is Asleep, Confused, or Paralyzed.</p>"#,
        );
        let expected = TextInfo::PokemonPower {
            name: "Energy Burn".to_string(),
            text: "As often as you like during your turn (before your attack), you may turn all Energy attached to Charizard into {R} Energy for the rest of the turn. This power can’t be used if Charizard is Asleep, Confused, or Paralyzed.".to_string(),
        };

        parse_text_info(expected, fragment);
    }

    #[test]
    fn parse_ancient_trait() {
        let fragment = Html::parse_fragment(
            r#"<p><a href="https://pkmncards.com/has/ancient-trait/">Ancient Trait</a> ⇢ <abbr title="Theta"><em>θ</em> </abbr> Stop<br>
 Prevent all effects of your opponent’s Pokémon’s Abilities done to this Pokémon.</p>"#,
        );
        let expected = TextInfo::AncientTrait {
            name: "θ  Stop".to_string(),
            text:
                "Prevent all effects of your opponent’s Pokémon’s Abilities done to this Pokémon."
                    .to_string(),
        };

        parse_text_info(expected, fragment);
    }

    #[test]
    fn parse_held_item() {
        let fragment = Html::parse_fragment(
            r#"<p><a href="https://pkmncards.com/has/held-item/">Held Item</a> ⇢ Magnet<br>
 Magnemite’s Retreat Cost is <abbr title="Colorless" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>C<span class="vh">}</span></abbr> less for each Magnemite on your Bench.</p>"#,
        );
        let expected = TextInfo::HeldItem {
            name: "Magnet".to_string(),
            text: "Magnemite’s Retreat Cost is {C} less for each Magnemite on your Bench."
                .to_string(),
        };
        parse_text_info(expected, fragment);
    }

    #[test]
    fn parse_attack_without_damage() {
        let fragment = Html::parse_fragment(
            r#"<p><abbr title="Grass" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>G<span class="vh">}</span></abbr> → <span>Sparkle Motion</span><br>
Put 1 damage counter on each of your opponent's Pokémon.</p>"#,
        );
        let expected = TextInfo::Attack {
            cost: vec![EnergyColor::Grass],
            name: "Sparkle Motion".to_string(),
            damage: None,
            text: "Put 1 damage counter on each of your opponent's Pokémon.".to_string(),
        };

        parse_text_info(expected, fragment);
    }

    #[test]
    fn parse_attack_with_damage() {
        let fragment = Html::parse_fragment(
            r#"<p><abbr title="Psychic" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>P<span class="vh">}</span></abbr><abbr title="Psychic" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>P<span class="vh">}</span></abbr><abbr title="Colorless" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>C<span class="vh">}</span></abbr>{+} → <span>Miraculous Duo-<em>GX</em></span> : 200<br>
If this Pokémon has at least 1 extra Energy attached to it <em>(in addition to this attack’s cost)</em>, heal all damage from all of your Pokémon. <em>(You can’t use more than 1 <em>GX</em> attack in a game.)</em></p>"#,
        );

        let expected = TextInfo::Attack {
            cost: vec![EnergyColor::Psychic, EnergyColor::Psychic, EnergyColor::Colorless],
            name: "Miraculous Duo-GX".to_string(),
            damage: Some("200".to_string()),
            text: "If this Pokémon has at least 1 extra Energy attached to it (in addition to this attack’s cost), heal all damage from all of your Pokémon. (You can’t use more than 1 GX attack in a game.)".to_string(),
        };

        parse_text_info(expected, fragment);
    }

    #[test]
    fn parse_rule() {
        let fragment = Html::parse_fragment(
            r#"<div class="rule tag-team">· <em>TAG TEAM <a href="https://pkmncards.com/has/rule-box/">rule</a>:</em> When your TAG TEAM is Knocked Out, your opponent takes 3 Prize cards.</div>"#,
        );
        let selector = Selector::parse("div").unwrap();

        let rule = Rule {
            purpose: "TAG TEAM".to_string(),
            text: "When your TAG TEAM is Knocked Out, your opponent takes 3 Prize cards."
                .to_string(),
        };

        assert_eq!(
            rule,
            Rule::parse(fragment.select(&selector).next().unwrap()).unwrap()
        );
    }

    #[test]
    fn parse_mark() {
        let mark = "F";
        assert_eq!(Mark::F, Mark::from_str(mark).unwrap());
    }

    #[test]
    fn parse_formats() {
        let fragment = Html::parse_fragment(
            r#"<span title="Format Type">Standard: <a href="https://pkmncards.com/format/upr-on-standard-2020/" title="UPR–on">2020</a>, <a href="https://pkmncards.com/format/teu-on-standard-2021/" title="TEU–on">2021</a></span>"#,
        );
        let selector = Selector::parse("span").unwrap();

        assert_eq!(
            Formats {
                format: FormatType::Standard,
                formats: vec![
                    PtcgFormat {
                        id: "UPR–on".to_string(),
                        text: "2020".to_string(),
                    },
                    PtcgFormat {
                        id: "TEU–on".to_string(),
                        text: "2021".to_string()
                    }
                ]
            },
            Formats::parse(fragment.select(&selector).next().unwrap()).unwrap()
        );
    }

    #[test]
    fn parse_format_type() {
        assert_eq!(
            FormatType::Standard,
            FormatType::from_str("Standard: ").unwrap()
        );
    }

    #[test]
    fn parse_ptcg_format() {
        let fragment = Html::parse_fragment(
            r#"<a href="https://pkmncards.com/format/upr-on-standard-2020/" title="UPR–on">2020</a>"#,
        );

        let selector = Selector::parse("a").unwrap();

        assert_eq!(
            PtcgFormat {
                id: "UPR–on".to_string(),
                text: "2020".to_string()
            },
            PtcgFormat::parse(fragment.select(&selector).next().unwrap()).unwrap()
        );
    }

    #[test]
    fn parse_energy_color() {
        assert_eq!(EnergyColor::Dark, EnergyColor::from_str("Dark").unwrap());
    }
}
