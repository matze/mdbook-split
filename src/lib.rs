use anyhow::Error;
use mdbook::book::{Book, BookItem, Chapter};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use pulldown_cmark::{Event, HeadingLevel, Tag};
use pulldown_cmark_to_cmark::cmark;
use sha2::Digest;
use std::path::PathBuf;

/// A no-op preprocessor.
pub struct Split;

impl Split {
    pub fn new() -> Split {
        Split
    }
}

fn is_h1(event: &Event) -> bool {
    matches!(event, Event::Start(Tag::Heading(HeadingLevel::H1, _, _)))
}

fn to_cmark(events: Vec<Event>) -> Result<String, Error> {
    let mut buf = String::new();
    cmark(events.into_iter(), &mut buf)?;
    Ok(buf)
}

fn to_chapter(events: Vec<Event>) -> Result<Chapter, Error> {
    let name = &events
        .windows(2)
        .find_map(|window| is_h1(&window[0]).then_some(&window[1]))
        .map(|event| match event {
            Event::Text(text) => Some(text.to_string()),
            _ => None,
        })
        .flatten()
        .unwrap_or_else(String::new);

    let content = to_cmark(events)?;

    let mut hasher = sha2::Sha256::new();
    hasher.update(&name);
    let result = hasher.finalize();

    Ok(Chapter {
        name: name.to_string(),
        path: Some(PathBuf::from(format!("{:x}", result))),
        content,
        ..Default::default()
    })
}

fn split_chapter(chapter: Chapter) -> Result<Vec<Chapter>, Error> {
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_FOOTNOTES);
    options.insert(pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION);
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);

    let parser = pulldown_cmark::Parser::new_ext(&chapter.content, options);
    let mut chapters = vec![];
    let mut events = vec![];

    for event in parser {
        let finish = is_h1(&event) && !events.is_empty();

        if finish {
            chapters.push(to_chapter(events)?);
            events = vec![event];
        } else {
            events.push(event);
        }
    }

    if !events.is_empty() {
        chapters.push(to_chapter(events)?);
    }

    Ok(chapters)
}

impl Preprocessor for Split {
    fn name(&self) -> &str {
        "split"
    }

    fn run(&self, _ctx: &PreprocessorContext, book: Book) -> Result<Book, Error> {
        let mut new_book = Book::new();

        for item in book.sections.into_iter() {
            match item {
                BookItem::Chapter(chapter) => {
                    for item in split_chapter(chapter)? {
                        new_book.push_item(item);
                    }
                }
                BookItem::Separator => {
                    new_book.push_item(BookItem::Separator);
                }
                BookItem::PartTitle(title) => {
                    new_book.push_item(BookItem::PartTitle(title));
                }
            }
        }

        Ok(new_book)
    }

    fn supports_renderer(&self, _renderer: &str) -> bool {
        true
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn nop_preprocessor_run() {
        let input_json = r##"[
                {
                    "root": "/path/to/book",
                    "config": {
                        "book": {
                            "authors": ["AUTHOR"],
                            "language": "en",
                            "multilingual": false,
                            "src": "src",
                            "title": "TITLE"
                        },
                        "preprocessor": {
                            "nop": {}
                        }
                    },
                    "renderer": "html",
                    "mdbook_version": "0.4.21"
                },
                {
                    "sections": [
                        {
                            "Chapter": {
                                "name": "Chapter 1",
                                "content": "# Chapter 1\n",
                                "number": [1],
                                "sub_items": [],
                                "path": "chapter_1.md",
                                "source_path": "chapter_1.md",
                                "parent_names": []
                            }
                        }
                    ],
                    "__non_exhaustive": null
                }
            ]"##;
        let input_json = input_json.as_bytes();

        let (ctx, book) = mdbook::preprocess::CmdPreprocessor::parse_input(input_json).unwrap();
        let expected_book = book.clone();
        let result = Split::new().run(&ctx, book);
        assert!(result.is_ok());

        // The nop-preprocessor should not have made any changes to the book content.
        let actual_book = result.unwrap();
        assert_eq!(actual_book, expected_book);
    }
}
