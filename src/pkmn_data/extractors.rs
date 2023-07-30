use anyhow::{Error, Result};
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
