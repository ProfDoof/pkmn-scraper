use anyhow::{anyhow, Error, Result};
use scraper::{ElementRef, Selector};

pub(super) fn clean_text<'a>(text: impl Iterator<Item = &'a str>) -> String {
    html_escape::decode_html_entities(
        &text
            .flat_map(|text| text.chars())
            .map(|ch| match ch {
                '’' => '\'',
                '“' | '”' => '"',
                c => c,
            })
            .collect::<String>(),
    )
    .trim()
    .to_string()
}

pub(super) fn extract_text(element: ElementRef) -> String {
    clean_text(element.text())
}

pub(super) fn direct_text_skip_past(element: ElementRef, pattern: &str) -> String {
    let mut iter = element.text().skip_while(|text| !text.contains(pattern));
    iter.next();
    clean_text(iter)
}

pub(super) fn select_text(element: ElementRef, selector: Selector) -> Result<String> {
    element
        .select(&selector)
        .next()
        .map(extract_text)
        .ok_or(Error::msg(format!(
            "Failed to extract text from {:?}: {}",
            selector,
            element.html()
        )))
}

pub(super) fn select_opt_text(element: ElementRef, selector: Selector) -> Option<String> {
    element.select(&selector).next().map(extract_text)
}

#[allow(dead_code)]
pub(super) fn select_title(element: ElementRef, selector: Selector) -> Result<String> {
    element
        .select(&selector)
        .next()
        .ok_or(Error::msg(format!(
            "Failed to extract title from {:?}: {}",
            selector,
            element.html()
        )))?
        .value()
        .attr("title")
        .ok_or(Error::msg(format!(
            "Failed to extract title from {:?}: {}",
            selector,
            element.html()
        )))
        .map(|title| title.to_string())
}

pub(super) fn select_element(element: ElementRef, selector: Selector) -> Result<ElementRef> {
    element.select(&selector).next().ok_or_else(|| {
        anyhow!(
            "Was not able to extract element using {:?} from: {}",
            selector,
            element.html()
        )
    })
}

pub(super) fn select_opt_element(element: ElementRef, selector: Selector) -> Option<ElementRef> {
    element.select(&selector).next()
}

pub(super) fn select_number(element: ElementRef, selector: Selector) -> Result<Option<i32>> {
    Ok(element
        .select(&selector)
        .next()
        .map(|elem| {
            elem.text()
                .collect::<String>()
                .trim_matches(|c| !char::is_numeric(c))
                .parse::<i32>()
        })
        .transpose()?)
}
