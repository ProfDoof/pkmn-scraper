use anyhow::{anyhow, Error, Result};
use scraper::{ElementRef, Selector};

pub(super) fn extract_text(element: ElementRef, selector: Selector) -> Result<String> {
    element
        .select(&selector)
        .next()
        .map(|set| set.text().collect::<String>())
        .ok_or(Error::msg(format!(
            "Failed to extract text from {:?}: {}",
            selector,
            element.html()
        )))
}

pub(super) fn extract_opt_text(element: ElementRef, selector: Selector) -> Option<String> {
    element
        .select(&selector)
        .next()
        .map(|elem| elem.text().collect::<String>())
}

#[allow(dead_code)]
pub(super) fn extract_title(element: ElementRef, selector: Selector) -> Result<String> {
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

pub(super) fn extract_element(element: ElementRef, selector: Selector) -> Result<ElementRef> {
    element.select(&selector).next().ok_or_else(|| {
        anyhow!(
            "Was not able to extract element using {:?} from: {}",
            selector,
            element.html()
        )
    })
}

pub(super) fn extract_opt_element(element: ElementRef, selector: Selector) -> Option<ElementRef> {
    element.select(&selector).next()
}

pub(super) fn extract_number(element: ElementRef, selector: Selector) -> Result<Option<i32>> {
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
