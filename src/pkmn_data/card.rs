use crate::pkmn_data::extractors::{
    extract_element, extract_number, extract_opt_element, extract_opt_text, extract_text,
};
use crate::ptcgio_data::Card;
use anyhow::{anyhow, bail, Context, Error, Result};
use ego_tree::NodeRef;
use regex::{Match, Regex};
use reqwest_middleware::ClientWithMiddleware;
use scraper::selector::CssLocalName;
use scraper::{ElementRef, Html, Node, Selector};
use selectors::attr::CaseSensitivity;
use selectors::Element;
use std::collections::{HashMap, HashSet};
use std::iter;
use std::ops::Deref;
use std::str::FromStr;
use strum::{Display, EnumString};
use time::macros::format_description;
use time::Date;

pub(super) struct CardFetcher {
    url: String,
    client: ClientWithMiddleware,
}

impl CardFetcher {
    pub(super) fn new(card_ref: ElementRef, client: &ClientWithMiddleware) -> Self {
        let url = card_ref.value().attr("href").unwrap().to_string();
        log::trace!("url for card: {}", url);
        Self {
            url,
            client: client.clone(),
        }
    }

    pub(super) async fn fetch(&self) -> Result<Card> {
        let response = self.client.get(&self.url).send().await?;
        let card_page = if response.status().is_success() {
            response.text().await?
        } else {
            bail!(
                "Failed to card info from {}: {}",
                &self.url,
                response.status()
            )
        };
        let entry_selector = Selector::parse("div.entry-content").unwrap();
        let html = Html::parse_document(&card_page);
        let elem = html
            .select(&entry_selector)
            .next()
            .ok_or(anyhow!("Could not retrieve page for {}", self.url))?;
        let card = CardText::parse(elem).with_context(|| format!("Failed to parse webpage: {}\n ##################################################################################\n{}\n##################################################################################", &self.url, elem.html()))?.try_into()?;

        Ok(card)
    }
}

#[derive(EnumString)]
enum Legality {
    #[strum(serialize = "Current")]
    Legal,
    Banned,
    #[strum(default)]
    OutOfFormat(String),
}

impl TryFrom<CardText> for Card {
    type Error = Error;

    fn try_from(value: CardText) -> Result<Self> {
        let mut abilities = value
            .all_text_info
            .text_infos
            .iter()
            .filter_map(|ability| {
                let a = ability.get_ability();
                match a {
                    Ok((a_type, a_name, a_text)) => Some(HashMap::from([
                        ("type".to_string(), a_type.0),
                        ("name".to_string(), a_name.0),
                        ("text".to_string(), a_text.0),
                    ])),
                    Err(_) => None,
                }
            })
            .collect::<Vec<HashMap<String, String>>>();

        let ancient_trait = abilities
            .iter()
            .position(|map| map.get("type").is_some_and(|t| t == "Ancient Trait"))
            .map(|idx| {
                let mut ancient_trait = abilities.remove(idx);
                ancient_trait.remove("type");
                ancient_trait
            });

        let rules = value.rules.map(|rules| {
            rules
                .rules
                .iter()
                .map(|rule| rule.text.clone())
                .chain(value.all_text_info.text_infos.iter().filter_map(
                    |text_info| match text_info {
                        TextInfo::Rule { rule } => Some(rule.clone()),
                        _ => None,
                    },
                ))
                .collect()
        });

        let attacks = value
            .all_text_info
            .text_infos
            .iter()
            .filter_map(|ability| match ability {
                TextInfo::Attack {
                    cost,
                    name,
                    damage,
                    text,
                } => Some(HashMap::from([
                    (
                        "cost".to_string(),
                        serde_json::Value::Array(
                            cost.iter()
                                .map(EnergyColor::to_string)
                                .map(serde_json::Value::String)
                                .collect::<Vec<serde_json::Value>>(),
                        ),
                    ),
                    (
                        "name".to_string(),
                        serde_json::Value::String(name.to_string()),
                    ),
                    (
                        "damage".to_string(),
                        serde_json::Value::String(damage.clone().unwrap_or("".to_string())),
                    ),
                    (
                        "text".to_string(),
                        serde_json::Value::String(text.to_string()),
                    ),
                    (
                        "convertedEnergyCost".to_string(),
                        serde_json::Value::Number(cost.len().into()),
                    ),
                ])),
                _ => None,
            })
            .collect::<Vec<HashMap<String, serde_json::Value>>>();

        let (from, to) = match value.type_evolves_is.evolves {
            None => (None, None),
            Some(evolves) => (
                evolves.from.first().map(|from| from.to_string()),
                Some(evolves.to),
            ),
        };

        let (weaknesses, resistances, retreat_cost, converted_retreat_cost) =
            match value.weak_resist_retreat {
                None => Ok::<_, Error>((None, None, None, None)),
                Some(wrr) => {
                    let extract_damage_modifiers = |damage_modifier: DamageModifier| {
                        if let Some(val) = damage_modifier.value {
                            Some(
                                damage_modifier
                                    .colors
                                    .iter()
                                    .map(PokeColor::to_string)
                                    .map(|color| {
                                        HashMap::from([
                                            ("type".to_string(), color),
                                            ("value".to_string(), val.clone()),
                                        ])
                                    })
                                    .collect::<Vec<HashMap<String, String>>>(),
                            )
                        } else {
                            None
                        }
                    };
                    let weaknesses = extract_damage_modifiers(wrr.weak);
                    let resistances = extract_damage_modifiers(wrr.resist);
                    let retreat_cost = iter::repeat("Colorless".to_string())
                        .take(wrr.retreat)
                        .collect::<Vec<String>>();
                    let cost = retreat_cost.len();
                    Ok((weaknesses, resistances, Some(retreat_cost), Some(cost)))
                }
            }?;

        let legalities = value
            .mark_formats
            .as_ref()
            .map(|mark_formats| {
                let legalities = HashMap::new();
                for format in &mark_formats.formats {}
                legalities
            })
            .unwrap_or(HashMap::new());

        Ok(Card {
            id: format!(
                "{}-{}",
                &value
                    .release_meta
                    .set_abbreviation
                    .unwrap_or("Needs manual tagging".to_string()),
                match &value.release_meta.set_number {
                    SetNumber::Num(num) => num.to_string(),
                    SetNumber::Str(s) => s.to_string(),
                }
            ),
            name: value.name_hp_color.name.clone(),
            supertype: value.type_evolves_is.pkmn_type.to_string(),
            subtypes: if let Some(subtype) = value.type_evolves_is.pkmn_subtype {
                if let Some(subsubtype) = value.type_evolves_is.pkmn_subsubtype {
                    Some(vec![subtype.to_string(), subsubtype.to_string()])
                } else {
                    Some(vec![subtype.to_string()])
                }
            } else {
                None
            },
            level: value.illus.level,
            hp: value.name_hp_color.hp.map(|hp| hp.to_string()),
            types: value
                .name_hp_color
                .color
                .map(|colors| colors.iter().map(|color| color.to_string()).collect()),
            evolves_from: from,
            evolves_to: to,
            abilities: if abilities.is_empty() {
                None
            } else {
                Some(abilities)
            },
            rules,
            attacks: if attacks.is_empty() {
                None
            } else {
                Some(attacks)
            },
            resistances,
            weaknesses,
            retreat_cost,
            converted_retreat_cost,
            number: "".to_string(),
            artist: value.illus.illustrator,
            rarity: Some(value.release_meta.rarity),
            flavor_text: value.flavor_text,
            national_pokedex_numbers: None,
            legalities,                 // TODO: Extract this from the formats data
            images: Default::default(), // This can be safely ignored
            ancient_trait,
            regulation_mark: value
                .mark_formats
                .and_then(|mark| mark.mark)
                .map(|mark| mark.to_string()),
        })
    }
}

trait PkmnParse {
    type Parsed;
    fn parse(element: ElementRef) -> Result<Self::Parsed>;
}

#[derive(Eq, PartialEq, Debug, EnumString, Display)]
enum PokeColor {
    Grass,
    Fire,
    Water,
    Lightning,
    Fighting,
    Psychic,
    Colorless,
    #[strum(serialize = "Dark", serialize = "Darkness")]
    Darkness,
    Metal,
    Dragon,
    Fairy,
    #[strum(default)]
    None(String),
}

#[derive(Eq, PartialEq, Debug)]
struct CardText {
    /// name-hp-color
    name_hp_color: NameHpColor,

    /// type-evolves-is
    type_evolves_is: TypeEvolvesIs,

    /// text
    all_text_info: AllTextInfo,

    /// weak-resist-retreat
    weak_resist_retreat: Option<WeakResistRetreat>,

    /// rules
    rules: Option<Rules>,

    /// illus
    illus: Illus,

    /// release-meta
    release_meta: ReleaseMeta,

    /// mark-formats
    mark_formats: Option<MarkFormats>,

    /// flavor
    flavor_text: Option<String>,
}

impl PkmnParse for CardText {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let name_hp_color_selector =
            Selector::parse("div.card-tabs > div.tab.text > div.name-hp-color").unwrap();
        let name_hp_color = extract_element(element, name_hp_color_selector)
            .and_then(NameHpColor::parse)
            .context("Failed to parse name-hp-color")?;

        let type_evolves_is_selector =
            Selector::parse("div.card-tabs > div.tab.text > div.type-evolves-is").unwrap();
        let type_evolves_is = extract_element(element, type_evolves_is_selector)
            .and_then(TypeEvolvesIs::parse)
            .context("Failed to parse type-evolves-is")?;

        let all_text_info_selector = Selector::parse("div.card-tabs > div.tab.text").unwrap();
        let all_text_info = extract_element(element, all_text_info_selector)
            .and_then(AllTextInfo::parse)
            .context("Failed to parse text")?;

        let weak_resist_retreat_selector =
            Selector::parse("div.card-tabs > div.tab.text > div.weak-resist-retreat").unwrap();
        let weak_resist_retreat = extract_opt_element(element, weak_resist_retreat_selector)
            .map(WeakResistRetreat::parse)
            .transpose()
            .context("Failed to parse weak-resist-retreat")?;

        let rules_selector = Selector::parse("div.card-tabs > div.tab.text > div.rules").unwrap();
        let rules = extract_opt_element(element, rules_selector)
            .map(Rules::parse)
            .transpose()
            .context("Failed to parse rules")?;

        let illus_selector = Selector::parse("div.card-tabs > div.tab.text > div.illus").unwrap();
        let illus = extract_element(element, illus_selector)
            .and_then(Illus::parse)
            .context("Failed to parse illus")?;

        let release_meta_selector =
            Selector::parse("div.card-tabs > div.tab.text > div.release-meta").unwrap();
        let release_meta = extract_element(element, release_meta_selector)
            .and_then(ReleaseMeta::parse)
            .context("Failed to parse release-meta")?;

        let mark_formats_selector =
            Selector::parse("div.card-tabs > div.tab.text > div.mark-formats").unwrap();
        let mark_formats = extract_opt_element(element, mark_formats_selector)
            .map(MarkFormats::parse)
            .transpose()
            .context("Failed to parse mark-formats")?;

        let flavor_text_selector =
            Selector::parse("div.card-tabs > div.tab.text > div.flavor").unwrap();
        let flavor_text = extract_opt_element(element, flavor_text_selector)
            .map(|elem| elem.text().collect::<String>());

        Ok(CardText {
            name_hp_color,
            type_evolves_is,
            all_text_info,
            weak_resist_retreat,
            rules,
            illus,
            release_meta,
            mark_formats,
            flavor_text,
        })
    }
}

#[derive(Eq, PartialEq, Debug)]
struct NameHpColor {
    name: String,
    hp: Option<i32>,
    color: Option<Vec<PokeColor>>,
}

impl PkmnParse for NameHpColor {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let name_selector = Selector::parse("span.name").unwrap();
        let name = extract_text(element, name_selector).context("Failed to parse name")?;

        let hp_selector = Selector::parse("span.hp").unwrap();
        let hp = extract_number(element, hp_selector).context("Failed to parse hp")?;

        let color_selector =
            Selector::parse("span.color > a > abbr.ptcg-font.ptcg-symbol-name").unwrap();
        let color = element
            .select(&color_selector)
            .map(|elem| {
                PokeColor::from_str(elem.value().attr("title").ok_or(anyhow!(
                    "Could not extract title from color: {}",
                    elem.html()
                ))?)
                .map_err(Error::from)
            })
            .collect::<Result<Vec<PokeColor>>>()
            .context("Failed to parse color")?;

        Ok(NameHpColor {
            name,
            hp,
            color: if color.is_empty() { None } else { Some(color) },
        })
    }
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
        let pkmn_type_selector = Selector::parse("span.type > a").unwrap();
        let pkmn_type = PkmnSuperType::from_str(
            &extract_text(element, pkmn_type_selector)
                .context("Failed to extract pkmn_super_type")?,
        )
        .context("Failed to convert pkmn_super_type")?;
        // Get Pkmn Subtype
        let pkmn_subtype_selector = Selector::parse("span.sub-type > a").unwrap();
        let mut pkmn_subtype_iter = element.select(&pkmn_subtype_selector);
        let (pkmn_subtype, pkmn_subsubtype) = if let Some(subtype) = pkmn_subtype_iter.next() {
            (
                Some(
                    PkmnSubtype::from_str(&subtype.text().collect::<String>())
                        .context("Failed to extract pkmn_subtype")?,
                ),
                pkmn_subtype_iter
                    .next()
                    .map(|subsubtype| {
                        PkmnSubSubType::from_str(&subsubtype.text().collect::<String>())
                    })
                    .transpose()
                    .context("Failed to extract pkmn_subsubtype")?,
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
            .transpose()
            .context("Failed to extract stage")?;

        // Get Evolves
        let evolves_selector = Selector::parse("span.evolves").unwrap();
        let evolves = element
            .select(&evolves_selector)
            .next()
            .map(Evolves::parse)
            .transpose()
            .context("Failed to extract evolves")?;

        // Get Is
        let is_selector = Selector::parse("span.is > a").unwrap();
        let is = element
            .select(&is_selector)
            .map(|elem| {
                let text = elem
                    .value()
                    .attr("href")
                    .ok_or(anyhow!("Failed to extract href from is tag"))?
                    .trim_start_matches("https://pkmncards.com/is/")
                    .trim_end_matches('/');

                PtcgTag::from_str(text).map_err(|err| {
                    anyhow!(
                        "Failed to extract a ptcg tag from \"{}\", Error: {}",
                        text,
                        err
                    )
                })
            })
            .collect::<Result<HashSet<PtcgTag>>>()
            .context("Failed to extract is tags")?;

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
                .collect::<Result<_>>()
                .context("Failed to parse text_infos in all_text_info")?,
        })
    }
}

#[derive(Eq, PartialEq, Debug)]
struct DamageModifier {
    colors: Vec<PokeColor>,
    value: Option<String>,
}

#[derive(Eq, PartialEq, Debug)]
struct WeakResistRetreat {
    weak: DamageModifier,
    resist: DamageModifier,
    retreat: usize,
}

impl PkmnParse for WeakResistRetreat {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        // get weak
        let weak_selector = Selector::parse("span.weak").unwrap();
        let weak_modifier_selector =
            Selector::parse("span.weak > span[title=\"Weakness Modifier\"]").unwrap();
        let weak = Self::get_damage_modifiers(element, &weak_selector, &weak_modifier_selector)
            .context("Failed to extract weaknesses")?;

        // get resist
        let resist_selector = Selector::parse("span.resist").unwrap();
        let resist_modifier_selector =
            Selector::parse("span.resist > span[title=\"Resistance Modifier\"]").unwrap();
        let resist =
            Self::get_damage_modifiers(element, &resist_selector, &resist_modifier_selector)
                .context("Failed to extract resistances")?;

        // get retreat
        let retreat_selector = Selector::parse("span.retreat > a > abbr").unwrap();
        let retreat = extract_opt_text(element, retreat_selector)
            .map(|text| text.parse::<usize>())
            .transpose()
            .context("Failed to extract retreat cost")?
            .unwrap_or(0);

        Ok(WeakResistRetreat {
            weak,
            resist,
            retreat,
        })
    }
}

impl WeakResistRetreat {
    fn get_damage_modifiers(
        element: ElementRef,
        damage_mod_selector: &Selector,
        modifier_selector: &Selector,
    ) -> Result<DamageModifier> {
        let damage_mod_elem = element.select(damage_mod_selector).next().ok_or(anyhow!(
            "Did not get damage modifier with {:?}",
            damage_mod_selector
        ))?;

        let color_selector = Selector::parse("a > abbr").unwrap();
        let colors = damage_mod_elem
            .select(&color_selector)
            .map(|elem| {
                elem.value()
                    .attr("title")
                    .ok_or(anyhow!("Element expected to have title did not"))
            })
            .map(|res| {
                res.and_then(|s| {
                    PokeColor::from_str(s).context("Failed to parse title to PokeColor")
                })
            })
            .collect::<Result<Vec<PokeColor>>>()?;

        let value = element
            .select(modifier_selector)
            .next()
            .map(|elem| elem.text().collect::<String>());

        Ok(DamageModifier { colors, value })
    }
}

#[derive(Eq, PartialEq, Debug)]
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
            .collect::<Result<_>>()
            .context("Failed to extract rule for rules")?;

        Ok(Rules { rules })
    }
}

#[derive(Eq, PartialEq, Debug)]
struct Illus {
    illustrator: Option<String>,
    level: Option<String>,
}

impl PkmnParse for Illus {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let illustrator_selector =
            Selector::parse("span[title=\"Illustrator\"] > a[title=\"Illustrator\"]").unwrap();

        let illustrator = extract_opt_text(element, illustrator_selector);

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
        match s
            .parse::<i32>()
            .map(SetNumber::Num)
            .map_err(|_| SetNumber::Str(s.to_string()))
        {
            Ok(sn) | Err(sn) => Ok(sn),
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
struct ReleaseMeta {
    series: Vec<String>,
    set: String,
    set_abbreviation: Option<String>,
    set_series_code: Option<String>,
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
        let set = extract_text(element, set_selector).context("Failed to extract set name")?;

        // Get Set Abbreviation
        let set_abbr_selector = Selector::parse("span[title=\"Set Abbreviation\"]").unwrap();
        let set_abbreviation = extract_opt_text(element, set_abbr_selector);

        // Get Set Code
        let set_series_code_selector = Selector::parse("span[title=\"Set Series Code\"]").unwrap();
        let set_series_code = extract_opt_text(element, set_series_code_selector);

        // Get Set Number
        let set_number_selector = Selector::parse("span.number-out-of > span.number").unwrap();
        let set_number = SetNumber::from_str(
            &extract_text(element, set_number_selector)
                .context("Failed to extract set_number text")?,
        )
        .context("Failed to convert set_number to SetNumber")?;

        // Get Total Cards in Set if available
        let set_total_cards_selector = Selector::parse("span.number-out-of > span.out-of").unwrap();
        let set_total_cards = extract_number(element, set_total_cards_selector)
            .context("failed to extract total cards in set")?;

        // Get Rarity
        let rarity_selector = Selector::parse("span.rarity > a[title=\"Rarity\"]").unwrap();
        let rarity = extract_text(element, rarity_selector).context("Failed to extract rarity")?;

        // Get Date Released
        let date_released_selector = Selector::parse("span.date[title=\"Date Released\"]").unwrap();
        let format_description = format_description!(
            version = 2,
            "↘ [month repr:short] [day padding:none], [year]"
        );

        let date_released = Date::parse(
            &extract_text(element, date_released_selector)
                .context("failed to extract date released text")?,
            format_description,
        )
        .context("Failed to convert date released into Date")?;

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
        let mark = extract_opt_element(element, mark_selector)
            .map(|mark| {
                let mark_str = mark.text().collect::<String>();
                Mark::from_str(&mark_str)
            })
            .transpose()
            .context("Failed to extract mark")?;
        let formats = element
            .select(&format_selector)
            .map(Formats::parse)
            .collect::<Result<Vec<Formats>>>()
            .context("Failed to extract formats for mark-formats")?;

        Ok(MarkFormats { mark, formats })
    }
}

#[derive(Eq, PartialEq, Debug, EnumString, Display)]
enum PkmnSuperType {
    #[strum(serialize = "Pokémon")]
    Pokemon,
    Trainer,
    Energy,
}

#[derive(Eq, PartialEq, Debug, EnumString, Display)]
enum PkmnSubtype {
    Item,
    Supporter,
    #[strum(serialize = "Basic Energy")]
    BasicEnergy,
    #[strum(serialize = "Pokémon Tool")]
    PokemonTool,
    Stadium,
    #[strum(serialize = "Special Energy")]
    SpecialEnergy,
    /// No longer considered a sub sub type as tools are no longer subtypes of items
    #[strum(serialize = "Pokémon Tool F")]
    PokemonToolF,
}

#[derive(Eq, PartialEq, Debug, EnumString, Display)]
enum PkmnSubSubType {
    #[strum(serialize = "Technical Machine")]
    TechnicalMachine,
    #[strum(serialize = "Rocket's Secret Machine")]
    RocketsSecretMachine,
    #[strum(serialize = "Goldenrod Game Corner")]
    GoldenrodGameCorner,
}

#[derive(Eq, PartialEq, Debug, EnumString, Display)]
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
            Regex::new(r#"(Evolves (from (?<from>(.+?)))?( and )?(into (?<to>(.+?)))?$)|(Put onto (?<put_onto>.+?)$)"#).unwrap();
        let text = element.text().collect::<String>();
        let caps = from_to_re.captures(&text).context(format!(
            "Could not extract the evolves to and from: {}",
            text
        ))?;

        let splitter_re = Regex::new(r#"((\s*or\s*)|(\s*,\s*))"#).unwrap();
        let split = |cap: Match| {
            splitter_re
                .split(cap.as_str())
                .filter_map(|mon| {
                    if mon.is_empty() {
                        None
                    } else {
                        Some(mon.to_string())
                    }
                })
                .collect::<Vec<String>>()
        };
        let from: Vec<String> = if let Some(cap) = caps.name("from") {
            split(cap)
        } else if let Some(cap) = caps.name("put_onto") {
            vec![cap.as_str().to_string()]
        } else {
            Vec::with_capacity(0)
        };

        let to: Vec<String> = if let Some(cap) = caps.name("to") {
            split(cap)
        } else {
            Vec::with_capacity(0)
        };

        Ok(Evolves { from, to })
    }
}

#[derive(Eq, PartialEq, Debug, Hash, EnumString, Display)]
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
    Rule {
        rule: String,
    },
}

struct AbilityName(String);
struct AbilityType(String);
struct AbilityText(String);

impl TextInfo {
    const ABILITY: &'static str = "Ability";
    const POKEBODY: &'static str = "Poké-BODY";
    const POKEPOWER: &'static str = "Poké-POWER";
    const POKEMON_POWER: &'static str = "Pokémon Power";
    const ANCIENT_TRAIT: &'static str = "Ancient Trait";
    const HELD_ITEM: &'static str = "Held Item";

    fn make_ability(
        ability_type: &str,
        ability_name: String,
        ability_text: String,
    ) -> Result<Self> {
        Ok(match ability_type {
            TextInfo::ABILITY => TextInfo::Ability {
                name: ability_name,
                text: ability_text,
            },
            TextInfo::POKEBODY => TextInfo::PokeBody {
                name: ability_name,
                text: ability_text,
            },
            TextInfo::POKEPOWER => TextInfo::PokePower {
                name: ability_name,
                text: ability_text,
            },
            TextInfo::POKEMON_POWER => TextInfo::PokemonPower {
                name: ability_name,
                text: ability_text,
            },
            TextInfo::ANCIENT_TRAIT => TextInfo::AncientTrait {
                name: ability_name,
                text: ability_text,
            },
            TextInfo::HELD_ITEM => TextInfo::HeldItem {
                name: ability_name,
                text: ability_text,
            },
            _ => Err(anyhow!("Unknown ability type: {}", ability_type))?,
        })
    }

    fn get_ability(&self) -> Result<(AbilityType, AbilityName, AbilityText)> {
        match self {
            TextInfo::Attack { .. } | TextInfo::Rule { .. } => {
                bail!("Attempt to get the ability from an attack or rule")
            }
            TextInfo::PokeBody { name, text }
            | TextInfo::HeldItem { name, text }
            | TextInfo::AncientTrait { name, text }
            | TextInfo::PokemonPower { name, text }
            | TextInfo::PokePower { name, text }
            | TextInfo::Ability { name, text } => Ok((
                AbilityType(self.get_ability_type()?),
                AbilityName(name.to_string()),
                AbilityText(text.to_string()),
            )),
        }
    }

    fn get_ability_type(&self) -> Result<String> {
        Ok(match self {
            TextInfo::Ability { .. } => TextInfo::ABILITY.to_string(),
            TextInfo::PokeBody { .. } => TextInfo::POKEBODY.to_string(),
            TextInfo::PokePower { .. } => TextInfo::POKEPOWER.to_string(),
            TextInfo::PokemonPower { .. } => TextInfo::POKEMON_POWER.to_string(),
            TextInfo::AncientTrait { .. } => TextInfo::ANCIENT_TRAIT.to_string(),
            TextInfo::HeldItem { .. } => TextInfo::HELD_ITEM.to_string(),
            TextInfo::Attack { .. } | TextInfo::Rule { .. } => {
                bail!("Tried to access ability type of an attack or rule")
            }
        })
    }

    fn get_text(element: ElementRef) -> Result<String, Error> {
        Ok(element
            .children()
            .skip_while(is_not_break)
            .map(read_text)
            .collect::<Result<Vec<Box<dyn Iterator<Item = &str>>>>>()
            .context("Failed to read text after the break element")?
            .into_iter()
            .flatten()
            .collect::<String>()
            .trim()
            .to_string())
    }

    fn get_string_til_break(element: ElementRef) -> Result<String, Error> {
        Ok(element
            .next_siblings()
            .map_while(read_text_til_break)
            .collect::<Result<Vec<Box<dyn Iterator<Item = &str>>>>>()
            .context("Failed to read text until the break element")?
            .into_iter()
            .flatten()
            .collect::<String>()
            .trim_start_matches([' ', '⇢', '→', '{', '}', '+'])
            .trim()
            .to_string())
    }

    fn get_cost(element: ElementRef) -> Result<(Option<ElementRef>, Vec<EnergyColor>), Error> {
        let energy_and_br_selector =
            Selector::parse("abbr.ptcg-font.ptcg-symbol-name, br, abbr[title=\"No Energy Cost\"]")
                .unwrap();

        let mut last_energy = None;
        let cost = element
            .select(&energy_and_br_selector)
            .map_while(|element| {
                if element.value().name.local == CssLocalName::from("br").0 {
                    None
                } else {
                    last_energy = Some(element);
                    if element.has_class(
                        &CssLocalName::from("ptcg-font"),
                        CaseSensitivity::CaseSensitive,
                    ) && element.has_class(
                        &CssLocalName::from("ptcg-symbol-name"),
                        CaseSensitivity::CaseSensitive,
                    ) {
                        Some(
                            EnergyColor::from_str(element.value().attr("title").unwrap())
                                .map_err(|err| anyhow!(err)),
                        )
                    } else {
                        None
                    }
                }
            })
            .collect::<Result<Vec<EnergyColor>>>()
            .context("Failed to extract cost of attack")?;
        Ok((last_energy, cost))
    }

    fn get_name_and_damage(
        html: String,
        last_energy: Option<ElementRef>,
    ) -> Result<(String, Option<String>), Error> {
        let name_and_damage = Self::get_string_til_break(
            last_energy.ok_or(anyhow!("Failed to extract name from: {}", html))?,
        )
        .context("Failed to extract name and damage string")?;

        let re = Regex::new(r#"^(?<name>.*?)(:\s*(?<damage>.*?)\s*)?$"#).unwrap();
        let captures = re
            .captures(&name_and_damage)
            .ok_or(anyhow!("Could not extract name or damage from: {}", html))?;
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
            _ => Err(anyhow!("Unknown node type")),
        }
    }
}

impl PkmnParse for TextInfo {
    type Parsed = Self;

    fn parse(element: ElementRef) -> Result<Self::Parsed> {
        let html = element.html();
        let mut children = element.children();
        let discrim_opt = ElementRef::wrap(
            children
                .next()
                .ok_or(anyhow!("No discriminator available: {}", html))?,
        );
        Ok({
            if let Some(discriminator) = discrim_opt {
                let local_name = &discriminator.value().name.local;
                if local_name == &CssLocalName::from("a").0 {
                    let ability_type = discriminator.inner_html();
                    let ability_name = Self::get_string_til_break(discriminator)
                        .context("Failed to extract ability name")?;
                    let ability_text =
                        Self::get_text(element).context("Failed to extract ability text")?;
                    TextInfo::make_ability(ability_type.as_str(), ability_name, ability_text)
                        .context("Failed to make ability")?
                } else if local_name == &CssLocalName::from("abbr").0 {
                    let (last_energy, cost) =
                        Self::get_cost(element).context("Failed to get cost and last_energy")?;
                    let (name, damage) = Self::get_name_and_damage(html, last_energy)
                        .context("Failed to get name and damage")?;
                    let text = Self::get_text(element).context("Failed to get attack text")?;
                    TextInfo::Attack {
                        cost,
                        name,
                        damage,
                        text,
                    }
                } else {
                    TextInfo::Rule {
                        rule: element.text().collect(),
                    }
                }
            } else {
                TextInfo::Rule {
                    rule: element.text().collect(),
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
        let purpose = extract_opt_element(element, purpose_selector)
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

#[derive(Eq, PartialEq, Debug, EnumString, Display)]
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
        let format_type = FormatType::from_str(element.text().next().unwrap())
            .context("Failed to parse format type")?;
        let formats = element
            .select(&format_selector)
            .map(PtcgFormat::parse)
            .collect::<Result<_>>()
            .context("Failed to parse formats")?;
        Ok(Formats {
            format: format_type,
            formats,
        })
    }
}

#[derive(Eq, PartialEq, Debug, EnumString, Display)]
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
            .ok_or(anyhow!("PtcgFormat did not have title: {}", html))?
            .to_string();
        let text: String = element.text().collect();
        Ok(PtcgFormat { id, text })
    }
}

#[derive(Eq, PartialEq, Debug, EnumString, Display)]
enum EnergyColor {
    Grass,
    Fire,
    Water,
    Lightning,
    Psychic,
    Fighting,
    Darkness,
    Metal,
    Fairy,
    Colorless,
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;
    use time::Month;

    #[test]
    fn test() {
        // let root = Selector::parse("html").unwrap();
        // let test = Selector::parse(":scope > p").unwrap();
        // let test_fragment = Html::parse_fragment(
        //     r#"
        // <p title="top">
        //     <p title="inner">ree</p>
        // </p>"#,
        // );
        // let start = test_fragment.select(&root).next().unwrap();
        // for elem in start
        //     .first_children()
        //     .flat_map(ElementRef::wrap)
        //     .filter(|elem| test.matches_with_scope(elem, Some(start)))
        // {
        //     println!("Element {:?}", elem);
        // }
        // panic!()
    }

    #[test]
    fn parse_card_text() {
        let fragment = Html::parse_fragment(
            r###"<main class="content" id="genesis-content"><article class="type-pkmn_card entry" id="post-66481"><div class="entry-content"><div class="card-image-area"><a href="https://i0.wp.com/pkmncards.com/wp-content/uploads/en_US-SM11-149-dragonair.jpg?fit=734%2C1024&amp;ssl=1" class="card-image-link" data-fancybox="img-zoom" data-width="734" data-height="1024" data-options="{&quot;classList&quot;:[&quot;card-image&quot;,&quot;box-shadow&quot;,&quot;skip-lazy&quot;],&quot;style&quot;:{&quot;background&quot;:&quot;rgb(255,228,109)&quot;}}"><img width="734" height="1024" src="https://i0.wp.com/pkmncards.com/wp-content/uploads/en_US-SM11-149-dragonair.jpg?fit=734%2C1024&amp;ssl=1" class="card-image box-shadow skip-lazy" alt="" decoding="async" loading="lazy" style="background:rgb(255,228,109)"></a><div class="image-meta"><ul><li class="zoom"><a href="https://i0.wp.com/pkmncards.com/wp-content/uploads/en_US-SM11-149-dragonair.jpg?fit=734%2C1024&amp;ssl=1" data-fancybox="zoom" data-width="734" data-height="1024" data-options="{&quot;classList&quot;:[&quot;card-image&quot;,&quot;box-shadow&quot;,&quot;skip-lazy&quot;],&quot;style&quot;:{&quot;background&quot;:&quot;rgb(255,228,109)&quot;}}">zoom <img src="https://s.w.org/images/core/emoji/14.0.0/svg/1f50d.svg" alt="🔍" class="wp-smiley" style="height: 1em; max-height: 1em;"></a></li><li><a href="https://pkmncards.com/wp-content/uploads/en_US-SM11-149-dragonair.jpg" title="Download Image" download="unm.149.dragonair.jpg">jpg (188 KB)</a></li><li title="Image Credit">cred: <a href="https://malie.io/" target="_blank"><span>nago</span></a></li></ul></div></div><div class="card-text-area"><header class="card-header"><div class="card-title-meta"><div class="wrap"><div class="card-title-admin-links"><h1 class="card-title" title="Title">Dragonair · Unified Minds (UNM) #149</h1></div><div class="card-meta"><ul><li class="proxy"><a href="https://pkmncards.com/proxy/?view=1&amp;back=66481" title="View Proxies">Proxy:</a> <ul><li><a href="https://pkmncards.com/proxy/?add=66481&amp;n=1&amp;back=66481" title="+1 Proxy">+<u>1</u></a></li><li><a href="https://pkmncards.com/proxy/?add=66481&amp;n=2&amp;back=66481" title="+2 Proxies">+<u>2</u></a></li><li><a href="https://pkmncards.com/proxy/?add=66481&amp;n=3&amp;back=66481" title="+3 Proxies">+<u>3</u></a></li><li><a href="https://pkmncards.com/proxy/?add=66481&amp;n=4&amp;back=66481" title="+4 Proxies">+<u>4</u></a></li></ul></li><li class="formats"><ul><li><a href="https://pkmncards.com/format/blw-on-expanded-current/" title="Legal for: Expanded"><img src="https://s.w.org/images/core/emoji/14.0.0/svg/1fa81.svg" alt="🪁" class="wp-smiley" style="height: 1em; max-height: 1em;"> <span>Expanded</span></a></li></ul></li><li class="views"><span class="flip-x" title="Views"><img src="https://s.w.org/images/core/emoji/14.0.0/svg/1f440.svg" alt="👀" class="wp-smiley" style="height: 1em; max-height: 1em;"> 388</span></li><li class="comments"><a href="https://pkmncards.com/card/dragonair-unified-minds-unm-149/#comments" title="Comments"><img src="https://s.w.org/images/core/emoji/14.0.0/svg/1f4ac.svg" alt="💬" class="wp-smiley" style="height: 1em; max-height: 1em;"> <span>3</span></a></li></ul></div></div></div><div class="card-pricing available"><div class="heading"><a href="https://www.tcgplayer.com/product/195144?partner=PkmnCards&amp;utm_source=PkmnCards&amp;utm_medium=single+66481+ago&amp;utm_campaign=affiliate" target="_blank">$ / TCGplayer (17 hours ago) <u>↗</u></a></div><div class="list"><ul><li class="l" title="Lowest Price">↓ <span class="price">0.09</span></li><li class="m" title="Market Price">꩜ <span class="price">0.21</span></li><li class="h" title="Highest Price">↑ <span class="price">4.99</span></li></ul></div></div></header><div class="card-tabs"><input class="toggle-tabs-rating vh" id="toggle-tabs-rating-66481" type="checkbox"><div class="tab text" id="text-66481"><div class="name-hp-color"><span class="name" title="Name"><a href="https://pkmncards.com/name/dragonair/">Dragonair</a></span> · <span class="hp" title="Hit Points"><a href="https://pkmncards.com/hp/90/">90 HP</a></span> · <span class="color" title="Color"><a href="https://pkmncards.com/color/dragon/"><abbr title="Dragon" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>N<span class="vh">}</span></abbr></a></span></div>
<div class="type-evolves-is"><span class="type" title="Type"><a href="https://pkmncards.com/type/pokemon/">Pokémon</a></span> <span class="pokemons">(<span class="pokemon" title="Pokémon"><a href="https://pkmncards.com/pokemon/dragonair/">Dragonair</a></span>)</span> › <span class="stage" title="Stage of Evolution"><a href="https://pkmncards.com/stage/stage-1/">Stage 1</a></span> : <span class="evolves">Evolves from <a href="https://pkmncards.com/name/dratini/" title="Name">Dratini</a> and into <a href="https://pkmncards.com/name/dragonite/" title="Name">Dragonite</a>, <a href="https://pkmncards.com/name/dragonite-gx/" title="Name">Dragonite-<em>GX</em></a>, or <a href="https://pkmncards.com/name/dragonite-ex-%e2%86%93/" title="Name">Dragonite ex</a></span></div>
<div class="text"><p><abbr title="Water" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>W<span class="vh">}</span></abbr><abbr title="Lightning" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>L<span class="vh">}</span></abbr> → <span>Twister</span> : 30<br>
Flip 2 coins. For each heads, discard an Energy from your opponent’s Active Pokémon. If both of them are tails, this attack does nothing.</p>
</div>
<div class="weak-resist-retreat"><span class="weak" title="Weakness">weak: <a href="https://pkmncards.com/weakness/fairy/"><abbr title="Fairy" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>Y<span class="vh">}</span></abbr></a><span title="Weakness Modifier">×2</span></span> | <span class="resist" title="Resistance">resist: <a href="https://pkmncards.com/?s=-resist%3A%2A"><abbr title="No Resistance">n/a</abbr></a></span> | <span class="retreat" title="Retreat Cost">retreat: <a href="https://pkmncards.com/retreat-cost/2/"><abbr title="{C}{C}">2</abbr></a></span></div>
<div class="illus minor-text"><span title="Illustrator">illus. <a href="https://pkmncards.com/artist/sanosuke-sakuma/" title="Illustrator">Sanosuke Sakuma</a></span></div>
<div class="release-meta minor-text"><span title="Series"><a href="https://pkmncards.com/series/sun-moon/" title="Series">Sun &amp; Moon</a></span> › <span title="Set"><a href="https://pkmncards.com/set/unified-minds/">Unified Minds</a></span> (<span title="Set Abbreviation">UNM</span>, <span title="Set Series Code">SM11</span>) › <span class="number-out-of">#<span class="number"><a href="https://pkmncards.com/number/149/" title="Number">149</a></span><span class="out-of" title="Out Of">/236</span></span> : <span class="rarity"><a href="https://pkmncards.com/rarity/uncommon/" title="Rarity">Uncommon</a></span> · <span class="date" title="Date Released">↘ Aug 2, 2019</span></div>
<div class="mark-formats minor-text"><span title="Legal Formats">Formats: <span title="Format Type">Standard: <a href="https://pkmncards.com/format/upr-on-standard-2020/" title="UPR–on">2020</a>, <a href="https://pkmncards.com/format/teu-on-standard-2021/" title="TEU–on">2021</a></span> · <span title="Format Type">Expanded: <a href="https://pkmncards.com/format/blw-on-expanded-2020/" title="BLW–on">2020</a>, <a href="https://pkmncards.com/format/blw-on-expanded-2021/" title="BLW–on">2021</a>, <a href="https://pkmncards.com/format/blw-on-expanded-current/" title="BLW–on">Current</a></span></span></div>
<div class="external-shop minor-text"><span title="External Links">External: <a href="https://www.pokemon.com/us/pokemon-tcg/pokemon-cards/sm1-series/sm11/149/" target="_blank">Pokemon.com ↗</a>, <a href="https://bulbapedia.bulbagarden.net/wiki/Dragonair_(Unified_Minds_149)" target="_blank" title="Bulbapedia">Bulba ↗</a></span> · <span title="Shopping Links">Shop: <a href="https://www.tcgplayer.com/product/195144?partner=PkmnCards&amp;utm_source=PkmnCards&amp;utm_medium=single+66481+shop&amp;utm_campaign=affiliate" target="_blank">TCGplayer ↗</a>, <a href="https://www.cardmarket.com/en/Pokemon/Products/Search?searchString=Dragonair+%28UNM+149%29&amp;referrer=pkmncards&amp;utm_source=pkmncards&amp;utm_medium=single+66481+&amp;utm_campaign=affiliate&amp;mode=gallery" target="_blank">cardmarket ↗</a>, <a href="https://www.ebay.com/sch/i.html?_nkw=%22dragonair%22+%22unified+minds%22+%22149%22&amp;mkcid=1&amp;mkrid=711-53200-19255-0&amp;siteid=0&amp;campid=5336458468&amp;customid=single66481shop&amp;toolid=10001&amp;mkevt=1" target="_blank">eBay ↗</a></span></div>
<div class="flavor minor-text">Lakes where Dragonair live are filled with offerings from people, because they believe this Pokémon is able to control the weather.</div></div></div></div></div></article><h2 class="screen-reader-text">Reader Interactions</h2><div class="entry-comments" id="comments"><h3 class="comments-title"><span>3 comments</span></h3><ol class="comment-list">
	<li class="comment even thread-even depth-1" id="comment-51226">
	<article id="article-comment-51226">

		<img alt="" src="https://secure.gravatar.com/avatar/4e8f0d6bcc0463db4e384bf7fee09c3e?s=50&amp;d=https%3A%2F%2Fpkmncards.com%2Fwp-content%2Fuploads%2Fclefairy-swing-2.jpg&amp;r=pg" class="avatar avatar-50 photo jetpack-lazy-image jetpack-lazy-image--handled" height="50" width="50" style="background:rgb(209,211,129)" decoding="async" srcset="https://secure.gravatar.com/avatar/4e8f0d6bcc0463db4e384bf7fee09c3e?s=100&amp;d=https%3A%2F%2Fpkmncards.com%2Fwp-content%2Fuploads%2Fclefairy-swing-2.jpg&amp;r=pg 2x" data-lazy-loaded="1" loading="eager"><noscript><img data-lazy-fallback="1" alt='' src='https://secure.gravatar.com/avatar/4e8f0d6bcc0463db4e384bf7fee09c3e?s=50&#038;d=https%3A%2F%2Fpkmncards.com%2Fwp-content%2Fuploads%2Fclefairy-swing-2.jpg&#038;r=pg' srcset='https://secure.gravatar.com/avatar/4e8f0d6bcc0463db4e384bf7fee09c3e?s=100&#038;d=https%3A%2F%2Fpkmncards.com%2Fwp-content%2Fuploads%2Fclefairy-swing-2.jpg&#038;r=pg 2x' class='avatar avatar-50 photo' height='50' width='50' style="background:rgb(209,211,129)" loading='lazy' decoding='async' /></noscript><div id="comment-wrap-51226" class="comment-wrap">
		<header class="comment-header">
			<p class="comment-author">
				<span class="comment-author-name">Ms Paint Pumpkin</span>			</p>

			<p class="comment-meta"><time class="comment-time"><a class="comment-time-link" href="https://pkmncards.com/card/dragonair-unified-minds-unm-149/#comment-51226" title="Wednesday, October 20, 2021 @ 11:48 PM EDT">(1 year ago)</a></time></p>		</header>

		<div class="comment-content">
			
			<p>Card Data says 100hp instead of 90</p>
		</div>

		<div class="comment-reply"><a rel="nofollow" class="comment-reply-link" href="#comment-51226" data-commentid="51226" data-postid="66481" data-belowelement="comment-wrap-51226" data-respondelement="respond" data-replyto="Reply to ↑" aria-label="Reply to ↑">Reply</a></div>
		</div>
	</article>
	<ul class="children">

	<li class="comment byuser comment-author-leo1532083 odd alt depth-2 user-role-subscriber user-role-moderator user-role-corrector" id="comment-51228">
	<article id="article-comment-51228">

		<img alt="" src="https://secure.gravatar.com/avatar/434da62eeb7e5c98927181b781e74d01?s=37&amp;d=https%3A%2F%2Fpkmncards.com%2Fwp-content%2Fuploads%2Fclefairy-swing-2.jpg&amp;r=pg" class="avatar avatar-37 photo jetpack-lazy-image jetpack-lazy-image--handled" height="37" width="37" style="background:rgb(209,211,129)" decoding="async" srcset="https://secure.gravatar.com/avatar/434da62eeb7e5c98927181b781e74d01?s=74&amp;d=https%3A%2F%2Fpkmncards.com%2Fwp-content%2Fuploads%2Fclefairy-swing-2.jpg&amp;r=pg 2x" data-lazy-loaded="1" loading="eager"><noscript><img data-lazy-fallback="1" alt='' src='https://secure.gravatar.com/avatar/434da62eeb7e5c98927181b781e74d01?s=37&#038;d=https%3A%2F%2Fpkmncards.com%2Fwp-content%2Fuploads%2Fclefairy-swing-2.jpg&#038;r=pg' srcset='https://secure.gravatar.com/avatar/434da62eeb7e5c98927181b781e74d01?s=74&#038;d=https%3A%2F%2Fpkmncards.com%2Fwp-content%2Fuploads%2Fclefairy-swing-2.jpg&#038;r=pg 2x' class='avatar avatar-37 photo' height='37' width='37' style="background:rgb(209,211,129)" loading='lazy' decoding='async' /></noscript><div id="comment-wrap-51228" class="comment-wrap">
		<header class="comment-header">
			<p class="comment-author" title="Registered User, Moderator">
				<span class="comment-author-name">LeoBN</span><span class="comment-in-reply-to" title="In Reply To"><a href="https://pkmncards.com/card/dragonair-unified-minds-unm-149/#comment-51226">Ms Paint Pumpkin</a></span>			</p>

			<p class="comment-meta"><time class="comment-time"><a class="comment-time-link" href="https://pkmncards.com/card/dragonair-unified-minds-unm-149/#comment-51228" title="Thursday, October 21, 2021 @ 8:18 AM EDT">(1 year ago)</a></time></p>		</header>

		<div class="comment-content">
			
			<p>Thanks. It is fixed now.</p>
		</div>

		<div class="comment-reply"><a rel="nofollow" class="comment-reply-link" href="#comment-51228" data-commentid="51228" data-postid="66481" data-belowelement="comment-wrap-51228" data-respondelement="respond" data-replyto="Reply to ↑" aria-label="Reply to ↑">Reply</a></div>
		</div>
	</article>
	</li><!-- #comment-## -->
</ul><!-- .children -->
</li><!-- #comment-## -->

	<li class="comment even thread-odd thread-alt depth-1" id="comment-51232">
	<article id="article-comment-51232">

		<img alt="" src="https://secure.gravatar.com/avatar/4e8f0d6bcc0463db4e384bf7fee09c3e?s=50&amp;d=https%3A%2F%2Fpkmncards.com%2Fwp-content%2Fuploads%2Fclefairy-swing-2.jpg&amp;r=pg" class="avatar avatar-50 photo jetpack-lazy-image jetpack-lazy-image--handled" height="50" width="50" style="background:rgb(209,211,129)" decoding="async" srcset="https://secure.gravatar.com/avatar/4e8f0d6bcc0463db4e384bf7fee09c3e?s=100&amp;d=https%3A%2F%2Fpkmncards.com%2Fwp-content%2Fuploads%2Fclefairy-swing-2.jpg&amp;r=pg 2x" data-lazy-loaded="1" loading="eager"><noscript><img data-lazy-fallback="1" alt='' src='https://secure.gravatar.com/avatar/4e8f0d6bcc0463db4e384bf7fee09c3e?s=50&#038;d=https%3A%2F%2Fpkmncards.com%2Fwp-content%2Fuploads%2Fclefairy-swing-2.jpg&#038;r=pg' srcset='https://secure.gravatar.com/avatar/4e8f0d6bcc0463db4e384bf7fee09c3e?s=100&#038;d=https%3A%2F%2Fpkmncards.com%2Fwp-content%2Fuploads%2Fclefairy-swing-2.jpg&#038;r=pg 2x' class='avatar avatar-50 photo' height='50' width='50' style="background:rgb(209,211,129)" loading='lazy' decoding='async' /></noscript><div id="comment-wrap-51232" class="comment-wrap">
		<header class="comment-header">
			<p class="comment-author">
				<span class="comment-author-name">Ms Paint Pumpkin</span>			</p>

			<p class="comment-meta"><time class="comment-time"><a class="comment-time-link" href="https://pkmncards.com/card/dragonair-unified-minds-unm-149/#comment-51232" title="Thursday, October 21, 2021 @ 2:36 PM EDT">(1 year ago)</a></time></p>		</header>

		<div class="comment-content">
			
			<p>Anytime</p>
		</div>

		<div class="comment-reply"><a rel="nofollow" class="comment-reply-link" href="#comment-51232" data-commentid="51232" data-postid="66481" data-belowelement="comment-wrap-51232" data-respondelement="respond" data-replyto="Reply to ↑" aria-label="Reply to ↑">Reply</a></div>
		</div>
	</article>
	</li><!-- #comment-## -->
</ol></div>	<div id="respond" class="comment-respond">
		<h3 id="reply-title" class="comment-reply-title">Join the Discussion <small><a rel="nofollow" id="cancel-comment-reply-link" href="/card/dragonair-unified-minds-unm-149/#respond" style="display:none;">Cancel reply</a></small></h3><form action="https://pkmncards.com/wp-comments-post.php" method="post" id="commentform" class="comment-form" novalidate=""><p class="comment-form-notes"><span class="note">Be kind; have fun.</span> <a href="https://pkmncards.com/wp-login.php?action=register">register</a> / <a href="https://pkmncards.com/wp-login.php?redirect_to=%2Fcard%2Fdragonair-unified-minds-unm-149%2F">log in</a></p><p class="comment-form-comment"><label class="vh" for="comment">Comment <span class="required">*</span></label> <textarea placeholder="Comment" id="comment" name="comment" cols="45" rows="4" maxlength="65525" required=""></textarea></p><p class="comment-form-author"><label class="vh" for="author">Name <span class="required">*</span></label> <input id="author" placeholder="Name" name="author" type="text" value="" size="30" maxlength="245" autocomplete="name" required=""></p>
<p class="comment-form-email"><label class="vh" for="email">Email <span class="required">*</span></label> <input id="email" placeholder="Email" name="email" type="email" value="" size="30" maxlength="100" autocomplete="email" required=""></p>
<p class="form-submit"><input name="submit" type="submit" id="submit" class="submit" value="Post Comment"> <input type="hidden" name="comment_post_ID" value="66481" id="comment_post_ID">
<input type="hidden" name="comment_parent" id="comment_parent" value="0">
</p><p style="display: none;"><input type="hidden" id="akismet_comment_nonce" name="akismet_comment_nonce" value="e766dc7f70"></p><p style="display: none !important;"><label>Δ<textarea name="ak_hp_textarea" cols="45" rows="8" maxlength="100"></textarea></label><input type="hidden" id="ak_js_1" name="ak_js" value="1691279559026"><script>document.getElementById( "ak_js_1" ).setAttribute( "value", ( new Date() ).getTime() );</script></p></form>	</div><!-- #respond -->
	</main>"###,
        );
        let selector = Selector::parse("div.entry-content").unwrap();
        let element = fragment.select(&selector).next().unwrap();
        let actual = CardText::parse(element).unwrap();
        let expected = CardText {
            name_hp_color: NameHpColor {
                name: "Dragonair".to_string(),
                hp: Some(90),
                color: Some(vec![PokeColor::Dragon]),
            },
            type_evolves_is: TypeEvolvesIs {
                pkmn_type: PkmnSuperType::Pokemon,
                pkmn_subtype: None,
                pkmn_subsubtype: None,
                all_pokemon: vec!["Dragonair".to_string()],
                stage: Some(Stage::Stage1),
                evolves: Some(Evolves {
                    from: vec!["Dratini".into()],
                    to: vec![
                        "Dragonite".into(),
                        "Dragonite-GX".into(),
                        "Dragonite ex".into(),
                    ],
                }),
                is: HashSet::default(),
            },
            all_text_info: AllTextInfo {
                text_infos: vec![TextInfo::Attack {
                    cost: vec![EnergyColor::Water, EnergyColor::Lightning],
                    name: "Twister".to_string(),
                    damage: Some("30".to_string()),
                    text: "Flip 2 coins. For each heads, discard an Energy from your opponent’s Active Pokémon. If both of them are tails, this attack does nothing.".to_string(),
                }],
            },
            weak_resist_retreat: Some(WeakResistRetreat {
                weak: DamageModifier{ colors: vec![PokeColor::Fairy], value: Some("×2".to_string()) },
                resist: DamageModifier{colors: vec![PokeColor::None("No Resistance".to_string())], value: None},
                retreat: 2,
            }),
            rules: None,
            illus: Illus {
                illustrator: Some("Sanosuke Sakuma".to_string()),
                level: None,
            },
            release_meta: ReleaseMeta {
                series: vec!["Sun & Moon".to_string()],
                set: "Unified Minds".to_string(),
                set_abbreviation: Some("UNM".to_string()),
                set_series_code: Some("SM11".to_string()),
                set_number: SetNumber::Num(149),
                set_total_cards: Some(236),
                rarity: "Uncommon".to_string(),
                date_released: Date::from_calendar_date(2019, Month::August, 2).unwrap(),
            },
            mark_formats: Some(MarkFormats {
                mark: None,
                formats: vec![Formats {
                    format: FormatType::Standard,
                    formats: vec![PtcgFormat {
                        id: "UPR–on".to_string(),
                        text: "2020".to_string(),
                    }, PtcgFormat {
                        id: "TEU–on".to_string(), 
                        text: "2021".to_string()}
                    ],
                }, Formats {
                    format: FormatType::Expanded,
                    formats: vec![PtcgFormat {
                        id: "BLW–on".to_string(),
                        text: "2020".to_string(),
                    }, PtcgFormat { id: "BLW–on".to_string(), text: "2021".to_string() }, PtcgFormat { id: "BLW–on".to_string(), text: "Current".to_string() }],
                }],
            }) ,
            flavor_text: Some("Lakes where Dragonair live are filled with offerings from people, because they believe this Pokémon is able to control the weather.".to_string()),
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_name_hp_color() {
        let fragment = Html::parse_fragment(
            r#"<div class="name-hp-color"><span class="name" title="Name"><a href="https://pkmncards.com/name/dragonair/">Dragonair</a></span> · <span class="hp" title="Hit Points"><a href="https://pkmncards.com/hp/90/">90 HP</a></span> · <span class="color" title="Color"><a href="https://pkmncards.com/color/dragon/"><abbr title="Dragon" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>N<span class="vh">}</span></abbr></a></span></div>"#,
        );
        let selector = Selector::parse("div").unwrap();
        let actual = NameHpColor::parse(fragment.select(&selector).next().unwrap()).unwrap();
        let expected = NameHpColor {
            name: "Dragonair".to_string(),
            hp: Some(90),
            color: Some(vec![PokeColor::Dragon]),
        };

        assert_eq!(actual, expected);

        let fragment = Html::parse_fragment(
            r#"<div class="name-hp-color"><span class="name" title="Name"><a href="https://pkmncards.com/name/jamming-net-team-flare-hyper-gear/">Jamming Net Team Flare Hyper Gear</a></span></div>"#,
        );
        let actual = NameHpColor::parse(fragment.select(&selector).next().unwrap()).unwrap();
        let expected = NameHpColor {
            name: "Jamming Net Team Flare Hyper Gear".to_string(),
            hp: None,
            color: None,
        };

        assert_eq!(actual, expected);
    }

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
    }

    #[test]
    fn parse_type_evolves_is_trainer() {
        let fragment = Html::parse_fragment(
            r#"<div class="type-evolves-is"><span class="type" title="Type"><a href="https://pkmncards.com/type/trainer/">Trainer</a></span> › <span class="sub-type" title="Sub-Type">(<a href="https://pkmncards.com/type/item/">Item</a>)</span> › <span class="sub-type" title="Sub-Type"><a href="https://pkmncards.com/type/rockets-secret-machine/">Rocket's Secret Machine</a></span></div>"#,
        );
        let selector = Selector::parse("div").unwrap();
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
    fn parse_type_evolves_is_level_x_pokemon() {
        let fragment = Html::parse_fragment(
            r#"<div class="type-evolves-is"><span class="type" title="Type"><a href="https://pkmncards.com/type/pokemon/">Pokémon</a></span> <span class="pokemons">(<span title="Pokémon" class="pokemon"><a href="https://pkmncards.com/pokemon/salamence/">Salamence</a></span>)</span> › <span title="Stage of Evolution" class="stage"><a href="https://pkmncards.com/stage/level-up/">Level-Up</a></span> : <span class="evolves">Put onto <a title="Name" href="https://pkmncards.com/name/salamence/">Salamence</a></span></div>"#,
        );
        let selector = Selector::parse("div").unwrap();
        let actual = TypeEvolvesIs::parse(fragment.select(&selector).next().unwrap()).unwrap();
        let expected = TypeEvolvesIs {
            pkmn_type: PkmnSuperType::Pokemon,
            pkmn_subtype: None,
            pkmn_subsubtype: None,
            all_pokemon: vec!["Salamence".to_string()],
            stage: Some(Stage::LevelUp),
            evolves: Some(Evolves {
                from: vec!["Salamence".to_string()],
                to: vec![],
            }),
            is: HashSet::default(),
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
    fn parse_text_info_murkrow() {
        let fragment = Html::parse_fragment(
            r#"<div class="text"><p><abbr class="ptcg-font ptcg-symbol-name" title="Colorless"><span class="vh">{</span>C<span class="vh">}</span></abbr> → <span>Spin Turn</span> : 10<br>
Switch this Pokémon with 1 of your Benched Pokémon.</p>
<p><abbr title="Darkness" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>D<span class="vh">}</span></abbr> → <span>United Wings</span> : 20×<br>
This attack does 20 damage for each Pokémon in your discard pile that has the United Wings attack.</p>
</div>"#,
        );
        let selector = Selector::parse("div").unwrap();
        let actual = AllTextInfo::parse(fragment.select(&selector).next().unwrap()).unwrap();
        let expected = AllTextInfo {
            text_infos: vec![
                TextInfo::Attack {
                    cost: vec![EnergyColor::Colorless],
                    name: "Spin Turn".to_string(),
                    damage: Some("10".to_string()),
                    text: "Switch this Pokémon with 1 of your Benched Pokémon.".to_string(),
                },
                TextInfo::Attack {
                    cost: vec![EnergyColor::Darkness],
                    name: "United Wings".to_string(),
                    damage: Some("20×".to_string()),
                    text: "This attack does 20 damage for each Pokémon in your discard pile that has the United Wings attack.".to_string(),
                },
            ],
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_weak_resist_retreat() {
        let fragment = Html::parse_fragment(
            r#"<div class="weak-resist-retreat"><span class="weak" title="Weakness">weak: <a href="https://pkmncards.com/weakness/psychic/"><abbr title="Psychic" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>P<span class="vh">}</span></abbr></a><span title="Weakness Modifier">×2</span></span> | <span class="resist" title="Resistance">resist: <a href="https://pkmncards.com/?s=-resist%3A%2A"><abbr title="No Resistance">n/a</abbr></a></span> | <span class="retreat" title="Retreat Cost">retreat: <a href="https://pkmncards.com/retreat-cost/2/"><abbr title="{C}{C}">2</abbr></a></span></div>"#,
        );
        let selector = Selector::parse("div").unwrap();
        let actual = WeakResistRetreat::parse(fragment.select(&selector).next().unwrap()).unwrap();
        let expected = WeakResistRetreat {
            weak: DamageModifier {
                colors: vec![PokeColor::Psychic],
                value: Some("×2".to_string()),
            },
            resist: DamageModifier {
                colors: vec![PokeColor::None("No Resistance".to_string())],
                value: None,
            },
            retreat: 2,
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_weak_resist_retreat_mew() {
        let fragment = Html::parse_fragment(
            r##"<div class="weak-resist-retreat"><span title="Weakness" class="weak">weak: <a href="https://pkmncards.com/weakness/darkness/"><abbr title="Darkness" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>D<span class="vh">}</span></abbr></a><span title="Weakness Modifier">×2</span></span> | <span title="Resistance" class="resist">resist: <a href="https://pkmncards.com/resistance/fighting/"><abbr title="Fighting" class="ptcg-font ptcg-symbol-name"><span class="vh">{</span>F<span class="vh">}</span></abbr></a><span title="Resistance Modifier">-30</span></span></div>"##,
        );
        let selector = Selector::parse("div").unwrap();
        let actual = WeakResistRetreat::parse(fragment.select(&selector).next().unwrap()).unwrap();
        let expected = WeakResistRetreat {
            weak: DamageModifier {
                colors: vec![PokeColor::Darkness],
                value: Some("×2".to_string()),
            },
            resist: DamageModifier {
                colors: vec![PokeColor::Fighting],
                value: Some("-30".to_string()),
            },
            retreat: 0,
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
            set_abbreviation: Some("SVP".to_string()),
            set_series_code: Some("Promo_SV".to_string()),
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
                    .ok_or(anyhow!("The checkbox did not contain a value"))?;
                PtcgTag::from_str(val).map_err(Error::from)
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
    fn parse_attack_with_no_cost() {
        let fragment = Html::parse_fragment(
            r#"<p><abbr title="No Energy Cost">{@}</abbr> → <span>Poison Breath</span><br>
       Flip a coin. If heads, the Defending Pokémon is now Poisoned.</p>"#,
        );
        let expected = TextInfo::Attack {
            cost: vec![],
            name: "Poison Breath".to_string(),
            damage: None,
            text: "Flip a coin. If heads, the Defending Pokémon is now Poisoned.".to_string(),
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
        assert_eq!(
            EnergyColor::Darkness,
            EnergyColor::from_str("Darkness").unwrap()
        );
    }
}
